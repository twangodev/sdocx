#!/usr/bin/env python3
"""Pinned native calligraphy saved redraw, queued upload, and Draw oracle.

Unicorn executes pinned ARM64 code. EGL/GLES imports and bitmap callbacks are
explicit synthetic host boundaries; this captures calls/bytes, not pixels.
"""
import argparse
import contextlib
import hashlib
import io
import json
import math
import re
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "conformance"))
import fountain_native
import calligraphy_saved_native as saved_calligraphy
from unicorn import arm64_const as R

LIB_SHA = saved_calligraphy.LIB_SHA
REDRAW = saved_calligraphy.REDRAW
CTOR = saved_calligraphy.CTOR
ADD = saved_calligraphy.ADD
END_PEN = saved_calligraphy.END_PEN
BUILD = saved_calligraphy.BUILD
LIBDIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
OBJDUMP = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump"
GRAPHICS_BASE = 0x4000000
UPDATE_BUFFER = "SPen::GLES::GraphicsObject::UpdateBuffer(unsigned int, void const*, unsigned int)"
DRAW = "SPen::MontblancCalligraphyPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)"

class NativeDrawCalligraphy(saved_calligraphy.NativeCalligraphy):
    """Saved calligraphy oracle plus bounded Renderer/GLES draw path."""

    def __init__(self, library_dir, objdump):
        renderer_sha = "f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed"
        graphics_sha = "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621"
        fountain_native.HASHES["Renderer"] = renderer_sha
        fountain_native.HASHES["Graphics"] = graphics_sha
        original_init = fountain_native.NativeFountain.__init__
        def load_render_dsos(self, directory, tool, additional_libraries=()):
            return original_init(self, directory, tool,
                                 (*additional_libraries, "Renderer", "Graphics"))
        fountain_native.NativeFountain.__init__ = load_render_dsos
        try:
            super().__init__(library_dir, objdump)
        finally:
            fountain_native.NativeFountain.__init__ = original_init
        self.graphics_base = self.syms["SPen::SPPenCanvas::SPPenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)"] - 0xae260
        self.graphics_host_methods = {}
        self.execute_move_pen = True
        self.stop_after_native_update = True
        self.recent_native_imports = []
        self.gl_calls = []
        self.gl_next_id = 100
        self.gl_strings = {}
        self.gl_locations = {}
        self.gl_shader_sources = {}
        self.gl_program_shaders = {}
        self.emutls = {}
        self.cxa_guards = set()
        self.queue_task_return = 0x20001010
        self.queue_runs = []
        self.pen_data_calls = []
        self.pending_pen_data_returns = {}
        self.pending_queue_return = 0
        self.pending_queue_saved_registers = []
        self.sample_ends = []
        self.terminal_start = None
        self.callbacks.update({
            "operator new(unsigned long, std::nothrow_t const&)": lambda: self.x(0, self.alloc(self.x(0))),
            "strlen": self.host_strlen, "__strlen_chk": self.host_strlen,
            "strstr": self.host_strstr, "strcmp": self.host_strcmp,
            "__emutls_get_address": self.host_emutls_get_address,
            "__strncpy_chk": self.host_strncpy_chk, "__strncpy_chk2": self.host_strncpy_chk,
            "pthread_mutex_init": lambda: self.x(0, 0),
            "pthread_mutex_lock": lambda: self.x(0, 0),
            "pthread_mutex_unlock": lambda: self.x(0, 0),
            "pthread_mutex_destroy": lambda: self.x(0, 0),
            "__cxa_guard_acquire": self.cxa_guard_acquire,
            "__cxa_guard_release": lambda: self.cxa_guards.add(self.x(0)),
            "__cxa_guard_abort": lambda: self.cxa_guards.discard(self.x(0)),
            "__cxa_atexit": lambda: self.x(0, 0),
            "SPen::PenDrawableRT::DisableDepthStencil()": lambda: None,
            "eglGetCurrentContext": self.host_no_egl_context,
            "__android_log_print": self.host_android_log, "crc32": self.host_crc32,
        })


    def cxa_guard_acquire(self):
        guard = self.x(0)
        if guard in self.cxa_guards:
            self.x(0, 0)
        else:
            self.cxa_guards.add(guard)
            self.x(0, 1)

    def host_strlen(self):
        pointer = self.x(0)
        length = 0
        try:
            while length < 4096 and self.u.mem_read(pointer + length, 1) != b"\0":
                length += 1
        except Exception as error:
            from unicorn import arm64_const as R
            caller = self.u.reg_read(R.UC_ARM64_REG_LR) - 4
            raise RuntimeError(f"strlen host boundary received unmapped {pointer:#x} at {caller:#x}; recent={self.recent_native_imports}") from error
        if length == 4096:
            raise RuntimeError("host strlen exceeded bound")
        self.x(0, length)

    def host_strstr(self):
        haystack, needle = self.x(0), self.x(1)
        hay = self.read_c_string(haystack).encode("utf-8")
        sub = self.read_c_string(needle).encode("utf-8")
        index = hay.find(sub)
        self.x(0, 0 if index < 0 else haystack + index)

    def host_strcmp(self):
        a, b = self.read_c_string(self.x(0)).encode(), self.read_c_string(self.x(1)).encode()
        self.x(0, (a > b) - (a < b))

    def host_emutls_get_address(self):
        descriptor = self.x(0)
        if descriptor not in self.emutls:
            size, alignment = self.get(descriptor, "QQ")
            if not 0 < size <= (1 << 20) or not 0 < alignment <= 4096:
                raise RuntimeError(f"invalid emutls descriptor {descriptor:#x}: {size=}, {alignment=}")
            allocation = self.alloc(size + alignment)
            address = (allocation + alignment - 1) & ~(alignment - 1)
            template = self.get(descriptor + 16, "Q")[0]
            if template:
                self.u.mem_write(address, bytes(self.u.mem_read(template, size)))
            self.emutls[descriptor] = address
        self.x(0, self.emutls[descriptor])

    def host_strncpy_chk(self):
        dest, src, count, capacity = self.x(0), self.x(1), self.x(2), self.x(3)
        if count > capacity or count > (1 << 20):
            raise RuntimeError(f"bounded strncpy overflow: {count=} {capacity=}")
        data = bytes(self.u.mem_read(src, count))
        end = data.find(b"\0")
        if end >= 0:
            data = data[:end] + bytes(count - end)
        self.u.mem_write(dest, data)
        self.x(0, dest)

    def host_sincos(self):
        from unicorn import arm64_const as R
        angle = struct.unpack("<d", struct.pack("<Q", self.u.reg_read(R.UC_ARM64_REG_D0)))[0]
        self.u.mem_write(self.x(0), struct.pack("<d", math.sin(angle)))
        self.u.mem_write(self.x(1), struct.pack("<d", math.cos(angle)))

    def host_no_egl_context(self):
        self.egl_context_queries = getattr(self, "egl_context_queries", 0) + 1
        self.x(0, 0)

    def host_android_log(self):
        # Android logging is an observable host boundary only; retain the
        # native format/value arguments and report a successful log write.
        self.android_logs = getattr(self, "android_logs", [])
        self.android_logs.append({"priority": self.x(0), "tag": self.x(1),
                                  "format": self.x(2), "arg0": self.x(3)})
        self.x(0, 0)

    def host_crc32(self):
        data = bytes(self.u.mem_read(self.x(1), self.x(2)))
        self.x(0, zlib.crc32(data, self.x(0)))

    def read_c_string(self, pointer, limit=1 << 20):
        result = bytearray()
        for offset in range(limit):
            char = self.u.mem_read(pointer + offset, 1)[0]
            if char == 0:
                return bytes(result).decode("utf-8", "replace")
            result.append(char)
        raise RuntimeError("GL host adapter string exceeded bound")

    def read_c_bytes(self, pointer, limit=1 << 20):
        result = bytearray()
        for offset in range(limit):
            char = self.u.mem_read(pointer + offset, 1)[0]
            if char == 0:
                return bytes(result)
            result.append(char)
        raise RuntimeError("GL host adapter source exceeded bound without NUL")

    def program_active_uniforms(self, program):
        import re
        gl_types = {"float": 0x1406, "int": 0x1404, "bool": 0x8B56,
                    "vec2": 0x8B50, "vec3": 0x8B51, "vec4": 0x8B52,
                    "mat4": 0x8B5C}
        sources = [source for shader in self.gl_program_shaders.get(program, [])
                   for source in self.gl_shader_sources.get(shader, [])]
        uniforms = {}
        pattern = r"\buniform\s+(float|int|bool|vec2|vec3|vec4|mat4)\s+(\w+)(?:\s*\[\s*(\d+)\s*\])?\s*;"
        for source in sources:
            for glsl_type, label, _array_size in re.findall(pattern, source):
                # The host adapter reflects only declared uniforms referenced
                # elsewhere in the pinned source, a bounded substitute for
                # driver reflection. Locations and uniform execution remain
                # synthetic host behavior.
                if len(re.findall(r"\b" + re.escape(label) + r"\b", source)) > 1:
                    uniforms.setdefault(label, gl_types[glsl_type])
        return sorted(uniforms.items())

    def gl_host_import(self, name):
        """Explicit host GL interface: native calls/state, synthetic driver objects."""
        x = self.x
        u = self.u
        args = [x(i) for i in range(6)]
        record = {"name": name, "args": args}
        self.gl_calls.append(record)
        if name == "eglGetCurrentContext":
            # Synthetic context token used only for native cache-key identity.
            self.egl_context_queries = getattr(self, "egl_context_queries", 0) + 1
            x(0, 0xE611)
        elif name == "glGetString":
            enum = x(0)
            payload = {0x1F00: b"sdocx host adapter", 0x1F01: b"synthetic GLES interface",
                       0x1F02: b"OpenGL ES 3.0 host adapter", 0x1F03: b""}.get(enum, b"")
            pointer = self.gl_strings.get(payload)
            if pointer is None:
                pointer = self.alloc(len(payload) + 1)
                u.mem_write(pointer, payload + b"\0")
                self.gl_strings[payload] = pointer
            x(0, pointer)
        elif name in {"glCreateShader", "glCreateProgram"}:
            handle = self.gl_next_id
            self.gl_next_id += 1
            record["handle"] = handle
            if name == "glCreateShader":
                self.gl_shader_sources[handle] = []
            else:
                self.gl_program_shaders[handle] = []
            x(0, handle)
        elif name == "glShaderSource":
            count, array, lengths = x(1), x(2), x(3)
            if count > 64:
                raise RuntimeError(f"unbounded glShaderSource count: {count}")
            sources = []
            for i in range(count):
                ptr = self.get(array + 8 * i, "Q")[0]
                length = self.get(lengths + 4 * i, "i")[0] if lengths else None
                if length is not None and length < 0:
                    raise RuntimeError("negative shader source length")
                if length is None:
                    data = self.read_c_bytes(ptr)
                else:
                    if length > 1 << 20:
                        raise RuntimeError(f"shader source exceeds bound: {length}")
                    data = bytes(u.mem_read(ptr, length)).split(b"\0", 1)[0]
                sources.append(data.decode("utf-8", "replace"))
            record["sources"] = sources
            self.gl_shader_sources[x(0)] = sources
        elif name in {"glCompileShader", "glLinkProgram", "glUseProgram", "glAttachShader",
                      "glBindAttribLocation", "glDeleteShader", "glDeleteProgram", "glActiveTexture",
                      "glBindBuffer", "glBindBufferBase", "glBindFramebuffer", "glBindRenderbuffer",
                      "glBindSampler", "glBindTexture", "glBlendEquationSeparate", "glBlendFunc",
                      "glBlendFuncSeparate", "glBufferData", "glClear", "glClearColor", "glClearDepthf",
                      "glClearStencil", "glColorMask", "glCullFace", "glDepthFunc", "glDepthMask",
                      "glDisable", "glDisableVertexAttribArray", "glDrawArrays", "glDrawArraysInstanced",
                      "glDrawElements", "glEnable", "glEnableVertexAttribArray", "glFramebufferRenderbuffer",
                      "glFramebufferTexture2D", "glPixelStorei", "glRenderbufferStorage", "glSamplerParameteri",
                      "glTexImage2D", "glTexParameteri", "glTexSubImage2D", "glUniform1i", "glUniform1f",
                      "glUniform2f", "glUniform3f", "glUniform4f", "glUniform1fv", "glUniform2fv",
                      "glUniform3fv", "glUniform4fv", "glUniformMatrix3fv", "glUniformMatrix4fv",
                      "glVertexAttribPointer", "glVertexAttribIPointer",
                      "glViewport", "glScissor", "glFinish", "glFlush", "glDeleteBuffers",
                      "glDeleteFramebuffers", "glDeleteRenderbuffers", "glDeleteSamplers", "glDeleteTextures",
                      "glDetachShader", "glCopyTexImage2D", "glCopyTexSubImage2D", "glInvalidateFramebuffer",
                      "glBindBuffer", "glSamplerParameteri"}:
            if name == "glBindAttribLocation":
                record["attribute_name"] = self.read_c_string(x(2))
            elif name == "glAttachShader":
                self.gl_program_shaders.setdefault(x(0), []).append(x(1))
            elif name in {"glBlendFuncSeparate", "glBlendEquationSeparate"}:
                record["values"] = args[:4]
            elif name in {"glDrawArrays", "glDrawArraysInstanced", "glDrawElements"}:
                record["draw"] = args[:5]
            elif name == "glVertexAttribPointer":
                record["attribute_pointer"] = {
                    "index": x(0), "size": x(1), "type": x(2),
                    "normalized": x(3), "stride": x(4), "buffer_offset": x(5)}
            elif name == "glBufferData":
                size, pointer = x(1), x(2)
                if size < 0 or size > 4 * 1024 * 1024:
                    raise RuntimeError(f"unsafe glBufferData size: {size}")
                record["buffer_data"] = {"target": x(0), "size": size,
                                         "usage": x(3),
                                         "bytes_hex": bytes(u.mem_read(pointer, size)).hex()
                                         if pointer and size else ""}
            elif name in {"glClearColor", "glUniform4f", "glUniform3f", "glUniform2f"}:
                record["float_bits"] = [f"{self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_S0 + i):08x}" for i in range(4)]
            elif name == "glUniform1f":
                record["float_bits"] = [f"{self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_S0):08x}"]
            elif name in {"glUniformMatrix3fv", "glUniformMatrix4fv"}:
                count = x(1)
                stride = 9 if name == "glUniformMatrix3fv" else 16
                if count > 64:
                    raise RuntimeError(f"unsafe {name} count: {count}")
                record["matrix_f32_bits"] = [
                    struct.pack("<f", value).hex()
                    for value in self.get(x(3), "f" * (count * stride))]
            elif name in {"glUniform1fv", "glUniform2fv", "glUniform3fv", "glUniform4fv"}:
                count = x(1)
                width = int(name[9])
                if count > 64:
                    raise RuntimeError(f"unsafe {name} count: {count}")
                record["float_f32_bits"] = [
                    struct.pack("<f", value).hex()
                    for value in self.get(x(2), "f" * (count * width))]
        elif name in {"glGetShaderiv", "glGetProgramiv"}:
            pname = x(1)
            if name == "glGetProgramiv" and pname == 0x8B86:
                self.active_uniforms = self.program_active_uniforms(x(0))
                result = len(self.active_uniforms)
            else:
                result = 1 if pname in (0x8B81, 0x8B82) else 0
            self.put(x(2), "i", result)
            record["written"] = {"pname": pname, "value": result}
        elif name == "glGetActiveUniform":
            index, capacity = x(1), x(2)
            uniforms = self.program_active_uniforms(x(0))
            if index >= len(uniforms):
                raise RuntimeError(f"unbounded glGetActiveUniform index {index}")
            label, gl_type = uniforms[index]
            encoded = label.encode("ascii")
            if capacity <= len(encoded):
                raise RuntimeError(f"glGetActiveUniform name buffer too small: {capacity}")
            self.u.mem_write(x(6), encoded + b"\0")
            if x(3): self.put(x(3), "i", len(encoded))
            if x(4): self.put(x(4), "i", 1)
            if x(5): self.put(x(5), "I", gl_type)
            record["uniform"] = {"index": index, "name": label, "gl_type": gl_type, "size": 1}
        elif name in {"glGetShaderInfoLog", "glGetProgramInfoLog"}:
            if x(2): self.put(x(2), "i", 0)
            if x(3): self.put(x(3), "B", 0)
        elif name in {"glGetAttribLocation", "glGetUniformLocation"}:
            label = self.read_c_string(x(1))
            key = (name, x(0), label)
            location = self.gl_locations.setdefault(key, len(self.gl_locations))
            record["label"] = label
            record["location"] = location
            x(0, location)
        elif name in {"glGenTextures", "glGenFramebuffers", "glGenRenderbuffers", "glGenBuffers", "glGenSamplers"}:
            count, output = x(0), x(1)
            if count > 256:
                raise RuntimeError(f"unbounded {name} count: {count}")
            handles = list(range(self.gl_next_id, self.gl_next_id + count))
            self.gl_next_id += count
            if handles: self.put(output, "I" * count, *handles)
            record["handles"] = handles
        elif name == "glCheckFramebufferStatus":
            x(0, 0x8CD5)
        elif name == "glGetError":
            x(0, 0)
        elif name == "glGetIntegerv":
            value = 4096 if x(0) == 0x0D33 else 16
            self.put(x(1), "i", value)
        elif name == "glGetBooleanv":
            self.put(x(1), "B", 0)
        elif name == "glGetVertexAttribiv":
            self.put(x(2), "i", 0)
        elif name in {"glIsSampler", "glIsTexture"}:
            x(0, 1)
        elif name == "glGetUniformBlockIndex":
            x(0, 0xFFFFFFFF)
        elif name == "glGetVertexAttribPointerv":
            self.put(x(2), "Q", 0)
        else:
            raise RuntimeError(f"unmodeled host GLES import {name}")
        self.u.reg_write(__import__("unicorn").arm64_const.UC_ARM64_REG_PC,
                         self.u.reg_read(__import__("unicorn").arm64_const.UC_ARM64_REG_LR))

    def drawable_state(self, drawable):
        fbits = lambda offset: struct.pack("<f", self.get(drawable + offset, "f")[0]).hex()
        return {"path_start_bits": [fbits(offset) for offset in (88, 92)],
                "control_bits": [fbits(offset) for offset in (96, 100)],
                "path_mid_bits": [fbits(offset) for offset in (104, 108)],
                "accepted_bits": [fbits(offset) for offset in (116, 120)],
                "direction_bits": [fbits(offset) for offset in (276, 280)],
                "angle_bits": struct.pack("<f", self.get(drawable + 284, "f")[0]).hex(),
                "flags": {"initial": self.get(drawable + 80, "B")[0],
                          "movement_armed": self.get(drawable + 272, "B")[0]}}

    def hook(self, u, address, size, data):
        self.last_native_pc = address
        move_pen = self.syms["SPen::MontblancCalligraphyPenStrokeDrawableGLV1::movePen(SPen::MotionEvent const*, SPen::RectF*)"]
        if self.execute_move_pen and address == move_pen + 0x3b4:
            # Preserve the native SendDataToGPU and PenGLRenderMsg::Run/Update
            # queue stages, then skip trailing density lookups which require a
            # real model/render context; use movePen's native epilogue.
            u.reg_write(__import__("unicorn").arm64_const.UC_ARM64_REG_PC, move_pen + 0x3e4)
            return
        # PenGLRenderMsg has executed the real RTV1::Update by +0x1f5b4.
        # Stop before canvas size metadata requests a real Android render target.
        if (self.execute_move_pen and address == self.syms[REDRAW] - 0x1e408 + 0x1f5b4
                and getattr(self, "stop_after_native_update", True)):
            from unicorn import arm64_const as R
            for register, value in self.pending_queue_saved_registers:
                u.reg_write(register, value)
            self.x(0, 1)  # Host queue accepted the native update task.
            u.reg_write(R.UC_ARM64_REG_PC, self.queue_task_return)
            return
        if address in self.plt:
            self.recent_native_imports.append(self.plt[address])
            self.recent_native_imports = self.recent_native_imports[-32:]
            if self.plt[address] == "__cxa_allocate_exception":
                print("native exception after imports", self.recent_native_imports, file=sys.stderr)
            if self.plt[address].startswith(("gl", "egl")):
                self.gl_host_import(self.plt[address])
                return
        if address in self.graphics_host_methods:
            result = self.graphics_host_methods[address]
            if callable(result): result(self)
            else: self.x(0, result)
            u.reg_write(__import__("unicorn").arm64_const.UC_ARM64_REG_PC, u.reg_read(__import__("unicorn").arm64_const.UC_ARM64_REG_LR))
            return
        if address in self.pending_pen_data_returns:
            rec = self.pending_pen_data_returns.pop(address)
            rec["after_raw"] = self.get(rec["receiver"], "I" * 16)
        if address in self.pending_length_returns:
            record = self.pending_length_returns.pop(address)
            record["length_bits"] = struct.pack("<f", self.f(0)).hex()
        if address in self.pending_position_returns:
            record = self.pending_position_returns.pop(address)
            record["ok"] = self.x(0) != 0
        if address == self.redraw_address:
            self.drawable_states.append({"phase": "redraw_entry", **self.drawable_state(self.x(0))})
        if address == self.redraw_address + 0x180:
            self.drawable_states.append({"phase": "history_loop_entry",
                                         **self.drawable_state(self.drawable)})
        if address == self.end_pen_address:
            self.drawable_states.append({"phase": "end_pen_entry",
                                         **self.drawable_state(self.x(0)),
                                         "event_end_bits": [struct.pack("<f", self.event_end["x"]).hex(),
                                                             struct.pack("<f", self.event_end["y"]).hex()],
                                         "size_bits": struct.pack("<f", self.current_size).hex()})
        if address == self.redraw_line_address and self.zero_length_probe and not self.zero_length_applied:
            drawable = self.x(0)
            # Synthetic state at the real redrawLine entry: force the actual
            # endpoint argument to equal the path's start/control point.
            self.f(0, 0.0)
            self.f(1, 0.0)
            self.f(2, 0.0)
            self.put(drawable + 88, "ff", 0.0, 0.0)
            self.put(drawable + 96, "ff", 0.0, 0.0)
            self.put(drawable + 104, "ff", 0.0, 0.0)
            self.put(drawable + 116, "ff", -2.0, 0.0)
            self.put(drawable + 276, "ff", 0.0, 1.0)
            self.put(drawable + 284, "f", 90.0)
            self.put(drawable + 80, "B", 0)
            self.put(drawable + 272, "B", 1)
            self.zero_length_applied = True
        if address == self.position_address:
            from unicorn import arm64_const as R
            caller = u.reg_read(R.UC_ARM64_REG_LR)
            record = {"caller_pc": hex(caller - 4),
                      "distance_bits": struct.pack("<f", self.f(0)).hex(),
                      "add_start": len(self.add_calls)}
            self.position_queries.append(record)
            self.pending_position_returns[caller] = record
        if address == self.length_address:
            from unicorn import arm64_const as R
            caller = u.reg_read(R.UC_ARM64_REG_LR)
            record = {"caller_pc": hex(caller - 4)}
            self.length_queries.append(record)
            self.pending_length_returns[caller] = record
        # RedrawPen has completed saved history and endPen at this boundary;
        # movePen consumes the separate trailing host event lifecycle.
        if address == self.syms[REDRAW] + 0x34c:
            if not self.execute_move_pen:
                from unicorn import arm64_const as R
                u.reg_write(R.UC_ARM64_REG_PC, 0x200f1000)
                u.emu_stop()
            # The saved geometry helper's hook has a geometry-only stop at
            # this boundary. Bypass that hook when continuing into movePen.
            return
        if address == 0x20001000:
            from unicorn import arm64_const as R
            caller = u.reg_read(R.UC_ARM64_REG_LR) - 4
            print("QUEUE_POST", hex(self.x(0)), hex(self.x(1)), hex(self.x(30)),
                  "from", hex(caller))
            task = self.x(1)
            print("TASK_WORDS", [hex(x) for x in self.get(task, "8Q")])
            vtable = self.get(task, "Q")[0]
            print("VT", hex(vtable), [hex(x) for x in self.get(vtable, "4Q")])
            method = self.get(vtable + 16, "Q")[0]
            self.queue_runs = getattr(self, "queue_runs", [])
            self.queue_runs.append({"task": task, "vtable": vtable, "run_method": method,
                                    "task_words": self.get(task, "8Q"), "vtable_words": self.get(vtable, "4Q")})
            if not method:
                raise RuntimeError(f"queue task has null Run: task={task:#x} vtable={vtable:#x} words={self.get(task, '8Q')} vtable_words={self.get(vtable, '4Q')}")
            self.pending_queue_return = caller + 4 if caller in {
                self.syms[REDRAW] - 0x1e408 + 0x1e2cc,
                self.syms[REDRAW] - 0x1e408 + 0x1e388,
            } else self.x(30)
            self.pending_queue_saved_registers = [(reg, u.reg_read(reg)) for reg in (
                R.UC_ARM64_REG_X19, R.UC_ARM64_REG_X20, R.UC_ARM64_REG_X21,
                R.UC_ARM64_REG_X22, R.UC_ARM64_REG_X23, R.UC_ARM64_REG_X24,
                R.UC_ARM64_REG_X25, R.UC_ARM64_REG_X26, R.UC_ARM64_REG_X27,
                R.UC_ARM64_REG_X28, R.UC_ARM64_REG_X29, R.UC_ARM64_REG_SP,
                R.UC_ARM64_REG_D8, R.UC_ARM64_REG_D9, R.UC_ARM64_REG_D10,
                R.UC_ARM64_REG_D11, R.UC_ARM64_REG_D12, R.UC_ARM64_REG_D13,
                R.UC_ARM64_REG_D14, R.UC_ARM64_REG_D15)]
            self.x(0, task)
            self.x(30, self.queue_task_return)
            u.reg_write(__import__("unicorn").arm64_const.UC_ARM64_REG_PC, method)
            return
        if address == getattr(self, "queue_task_return", None):
            u.reg_write(__import__("unicorn").arm64_const.UC_ARM64_REG_PC, self.pending_queue_return)
            return
        if address == self.syms["SPen::PenDrawableRT::SetPenData(float, int)"]:
            from unicorn import arm64_const as R
            self.pen_data_calls.append({"receiver": self.x(0), "size_bits": struct.pack("<f", self.f(0)).hex(),
                                       "argb": self.x(1), "before": self.drawable_state(self.drawable)})
            self.pending_pen_data_returns[u.reg_read(R.UC_ARM64_REG_LR)] = self.pen_data_calls[-1]
        if address == self.add_address:
            if self.direct_extent_probe and not self.add_calls:
                # Exercise the native AddPoint clamp with an explicit S0 input,
                # while keeping RedrawPen's real drawable and output buffers.
                self.f(0, 0.5)
            vector = self.x(1)
            if self.active_add_index is not None:
                self.add_calls[self.active_add_index]["build_calls"] = len(self.build_calls) - self.add_calls[self.active_add_index]["build_start"]
            drawable = self.x(0)
            self.active_add_index = len(self.add_calls)
            self.add_calls.append({"vector_bits": [self.get(vector + 4 * i, "I")[0].to_bytes(4, "little").hex() for i in range(4)],
                                   "extent_bits": struct.pack("<f", self.f(0)).hex(),
                                   "pre_state": {"center_bits": [struct.pack("<f", self.get(drawable + offset, "f")[0]).hex() for offset in (296, 300)],
                                                 "initial_flag": self.get(drawable + 304, "B")[0],
                                                 "nib_bits": [self.get(self.drawable + offset, "I")[0].to_bytes(4, "little").hex() for offset in (276, 280, 284)],
                                                 "edge_state_bits": [self.get(drawable + 308 + 4 * i, "I")[0].to_bytes(4, "little").hex() for i in range(12)]},
                                   "build_start": len(self.build_calls)})
        if address == self.syms[REDRAW] + 0x1cc:
            from unicorn import arm64_const as R
            self.sample_ends.append({"history_index": u.reg_read(R.UC_ARM64_REG_W23),
                                     "add_point_end": len(self.add_calls)})
        if address == self.syms[REDRAW] + 0x300:
            self.terminal_start = len(self.add_calls)
        if address == self.syms[REDRAW] + 0x170:
            rtv1 = self.get(self.drawable + 16, "Q")[0]
            self.output_labels = {self.get(rtv1 + offset, "Q")[0]: f"mesh_{offset}"
                                  for offset in (200, 208, 216)}
        if address == self.build_address:
            rect = self.x(1)
            buffer = self.output_labels.get(self.x(2))
            if buffer not in {"mesh_200", "mesh_208", "mesh_216"}:
                raise RuntimeError(f"buildMesh output vector was not associated with a known RTV1 member: {self.x(2):#x}")
            self.build_calls.append({"rect_bits": [self.get(rect + 4 * i, "I")[0].to_bytes(4, "little").hex() for i in range(8)],
                                     "buffer": buffer})
            if self.direct_extent_probe or self.zero_length_probe:
                from unicorn import arm64_const as R
                if self.direct_extent_probe and len(self.build_calls) == 5:
                    u.reg_write(R.UC_ARM64_REG_PC, 0x200f1000)
                    u.emu_stop()
                else:
                    u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
                return
        if address in self.plt and self.plt[address] == "SPen::MotionEvent::GetHistoricalOrientation(int) const":
            self.orientation_reads.append(self.x(1))
        super().hook(u, address, size, data)

    def run(self, case):
        self.history = [tuple(row) for row in case["history"]]
        last = case["end"] if "end" in case else self.history[-1]
        self.event_end = {"x": last[0], "y": last[1], "orientation": last[2],
                          "pressure": case.get("pressure", 0.5), "tilt": case.get("tilt", 0.0),
                          "time": case.get("time", 100), "tool": case.get("tool", 2)}
        self.current_size = case.get("size", 4.0)
        self.add_calls = []
        self.orientation_reads = []
        self.build_calls = []
        self.sample_ends = []
        self.terminal_start = None
        self.drawable_states = []
        self.position_queries = []
        self.length_queries = []
        self.pending_position_returns = {}
        self.pending_length_returns = {}
        data = self.alloc(256)
        manager = self.alloc(64)
        self.queue = self.alloc(64)
        qvt = self.alloc(32)
        self.put(self.queue, "Q", qvt)
        self.put(qvt + 16, "Q", 0x20001000)
        # Apply only the pinned GOT entries exercised by the real manager and canvas constructors.
        cbase = 0x1000000
        gbase = self.graphics_base
        self.put(cbase + 0x7a3f8, "Q", cbase + 0x76d58)
        calligraphy_base = self.redraw_address - 0x1e408
        self.put(calligraphy_base + 0x2ba98, "Q", calligraphy_base + 0x114a3)
        self.put(calligraphy_base + 0x2baa8, "Q", calligraphy_base + 0x114cb)
        self.put(calligraphy_base + 0x2ba40, "Q", calligraphy_base + 0x2a768)
        self.put(calligraphy_base + 0x2ba88, "Q", calligraphy_base + 0x20eec)
        self.put(calligraphy_base + 0x2a778, "Q", calligraphy_base + 0x1d458)
        self.put(calligraphy_base + 0x2a780, "Q", calligraphy_base + 0x1f4f8)
        self.put(calligraphy_base + 0x2a788, "Q", calligraphy_base + 0x1f4fc)
        for got_offset, source_offset in ((0x2baf8, 0x11517), (0x2bb00, 0x11614),
                                          (0x2bb08, 0x11710), (0x2bb10, 0x117c1),
                                          (0x2bb88, 0x11bf0), (0x2bb90, 0x11d11)):
            self.put(calligraphy_base + got_offset, "Q", calligraphy_base + source_offset)
        self.put(calligraphy_base + 0x2bab8, "Q", calligraphy_base + 0x2a8b0)
        self.put(calligraphy_base + 0x2a8f0, "Q", calligraphy_base + 0x1fbc4)
        self.put(calligraphy_base + 0x2a908, "Q", calligraphy_base + 0x20390)
        self.put(gbase + 0xd6cd8, "Q", gbase + 0xd1120)
        self.put(gbase + 0xd6cd0, "Q", gbase + 0xd0cb8)
        self.put(gbase + 0xd0d80, "Q", gbase + 0xaef00)
        self.put(gbase + 0xd0d88, "Q", gbase + 0xaef08)
        self.put(gbase + 0xd0d90, "Q", gbase + 0xaef14)
        self.put(gbase + 0xd0da0, "Q", gbase + 0xae4dc)
        # EnableClipRT creates a renderer/rasterizer target. This probe has no
        # native ContextHolder/real GPU-backed canvas target beyond the host
        # adapter, so this one canvas-side state transition is an explicit stub.
        clip_enable_stub = 0x20005008
        self.graphics_host_methods[clip_enable_stub] = 0
        self.put(gbase + 0xd0db0, "Q", clip_enable_stub)
        self.put(gbase + 0xd6d30, "Q", gbase + 0xd0fa8)
        self.put(gbase + 0xd6d28, "Q", gbase + 0xafb4c)
        self.put(gbase + 0xd6ce0, "Q", gbase + 0xd0de0)
        self.put(gbase + 0xd7338, "Q", gbase + 0x5b244)
        self.put(gbase + 0xd7320, "Q", gbase + 0x5af32)
        self.put(gbase + 0xd7330, "Q", gbase + 0x5b0f5)
        self.put(gbase + 0xd6d38, "Q", gbase + 0xd0fc8)
        self.put(gbase + 0xd0fd8, "Q", gbase + 0x85f14)
        self.put(gbase + 0xd0fe0, "Q", gbase + 0xb002c)
        self.put(gbase + 0xd0fe8, "Q", gbase + 0xb0030)
        self.put(cbase + 0x7a420, "Q", cbase + 0x77b98)
        self.put(cbase + 0x77ba8, "Q", cbase + 0x43a7c)
        self.put(cbase + 0x77bb0, "Q", cbase + 0x4a270)
        self.put(cbase + 0x77bb8, "Q", cbase + 0x4a274)
        manager_ctor = "SPen::PenGLDataManager::PenGLDataManager()"
        self.call(manager_ctor, [manager])
        self.call("SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)", [manager, self.queue])
        renderer_base = 0x4000000
        # Resolve only the genuine pinned ContextHolder vtable relocation used
        # by ShaderManager::Key. Its virtual GetCurrentContextId reaches the
        # explicitly bounded eglGetCurrentContext host callback above.
        self.put(renderer_base + 0x6d630, "Q", renderer_base + 0x6a0a0)
        self.put(renderer_base + 0x6a0c0, "Q", renderer_base + 0x5157c)
        self.put(renderer_base + 0x6d4a8, "Q", renderer_base + 0x73288)
        self.put(renderer_base + 0x6d4b0, "Q", renderer_base + 0x73290)
        self.put(renderer_base + 0x6d4a0, "Q", renderer_base + 0x302c8)
        # Resolve actual GLES implementation vtables used by RTV1::Init.
        # NativeFountain maps ELF bytes but does not apply these GLOB_DATs.
        self.put(renderer_base + 0x6d620, "Q", renderer_base + 0x6a068)
        self.put(renderer_base + 0x6d650, "Q", renderer_base + 0x6a210)
        # GetBindingIntf allocates the native ParameterBindingImpl and stores
        # its vptr as this GOT target + 0x10. NativeFountain maps ELF segments
        # but does not apply the two GLOB_DAT relocations, leaving the object
        # with the invalid first word 0x10 unless these are resolved.
        self.put(renderer_base + 0x6d428, "Q", renderer_base + 0x69ca0)
        self.put(renderer_base + 0x6d6d0, "Q", renderer_base + 0x6a608)
        self.put(renderer_base + 0x6a618, "Q", renderer_base + 0x55954)
        self.put(renderer_base + 0x6a620, "Q", renderer_base + 0x55ca0)
        for slot, method in enumerate((0x530a0, 0x530e8, 0x52218, 0x52380,
                                       0x52a10, 0x52824, 0x52464, 0x5264c,
                                       0x53134, 0x53140, 0x53224)):
            self.put(renderer_base + 0x6a220 + slot * 8, "Q", renderer_base + method)
        for slot, method in enumerate((0x51490, 0x51494, 0x514b8, 0x514cc, 0x51570)):
            self.put(renderer_base + 0x6a078 + slot * 8, "Q", renderer_base + method)
        self.put(renderer_base + 0x6d890, "Q", renderer_base + 0x6ae90)
        for table_offset, method_offset in ((0x6aea0, 0x5b9c8), (0x6aea8, 0x5ba20),
                                            (0x6aeb0, 0x5b824), (0x6aeb8, 0x5b9bc),
                                            (0x6aec0, 0x5b9c4), (0x6aec8, 0x5ba6c),
                                            (0x6aed0, 0x5b2c4), (0x6aed8, 0x5c5a8),
                                            (0x6aee0, 0x5c600), (0x6af08, 0x45600),
                                            (0x6af10, 0x4563c), (0x6af18, 0x5c5d4),
                                            (0x6af20, 0x5c604)):
            self.put(renderer_base + table_offset, "Q", renderer_base + method_offset)
        shader_manager = self.alloc(512)
        self.put(renderer_base + 0x6d480, "Q", shader_manager)
        self.call("SPen::ShaderManager::ShaderManager()", [shader_manager], instruction_limit=2_000_000)
        self.call("SPen::ShaderManager::GetInstance()", [])
        print("SHADER_MANAGER", hex(shader_manager), hex(self.x(0)), hex(self.get(renderer_base + 0x6d480, "Q")[0]))
        # A native SPPenCanvas is built around a narrowly explicit host bitmap interface.
        bitmap = self.alloc(64); bitmap_vtable = self.alloc(192)
        surface = self.alloc(32); surface_vtable = self.alloc(128)
        bitmap_stub_base = 0x20003000
        for index, val in ((16, 1), (64, surface), (72, 1), (80, 1),
                           (128, 1), (160, surface), (168, surface)):
            stub = bitmap_stub_base + index
            self.graphics_host_methods[stub] = val
            self.put(bitmap_vtable + index, "Q", stub)
        surface_stub = bitmap_stub_base + 0x200
        self.graphics_host_methods[surface_stub] = 1
        self.put(surface_vtable + 16, "Q", surface_stub)
        self.put(surface_vtable + 72, "Q", surface_stub)
        for index, value in ((80, 128), (88, 128), (96, 1)):
            stub = bitmap_stub_base + 0x200 + index
            self.graphics_host_methods[stub] = value
            self.put(surface_vtable + index, "Q", stub)
        self.put(bitmap, "Q", bitmap_vtable); self.put(surface, "Q", surface_vtable)
        canvas = self.alloc(320)
        self.call("SPen::SPPenCanvas::SPPenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)", [canvas, self.queue, bitmap], instruction_limit=2_000_000)
        drawable = self.alloc(512)
        self.drawable = drawable
        self.call(CTOR, [drawable, data, manager])
        print("RT_OBJECT", hex(self.get(drawable + 16, "Q")[0]), self.get(self.get(drawable + 16, "Q")[0], "16Q"))
        self.call("SPen::PenStrokeDrawableGL::SetCanvas(SPen::ISPPenCanvas*)", [drawable, canvas], instruction_limit=2_000_000)
        # V1 reads pen size and optional fixed-width state through PenData.
        settings = self.alloc(64)
        self.put(data, "Q", settings)
        self.put(settings, "fI", self.current_size, case.get("color", 0xff000000))
        # Resolve only the three pinned worker imports/vtables used by this
        # redraw tail. Member2::Run is the real local thunk at 0x1f978.
        base = self.syms[REDRAW] - 0x1e408
        self.put(base + 0x2ba48, "Q", base + 0x2ab58)
        for offset, method in ((0x2ab68, 0x1d458), (0x2ab70, 0x1f524), (0x2ab78, 0x1f528)):
            self.put(base + offset, "Q", base + method)
        self.put(base + 0x2a920, "Q", self.syms["SPen::MontblancCalligraphyPenStrokeDrawableRTV1::Update(bool)"])
        self.put(base + 0x2ba60, "Q", base + 0x2abe8)
        self.put(base + 0x2ba68, "Q", base + 0x2a7c8)
        self.put(base + 0x2ba78, "Q", self.syms["SPen::PenDrawableRT::SetRedrawState(bool)"])
        self.put(base + 0x2ba80, "Q", self.syms["SPen::PenDrawableRT::SetPenData(float, int)"])
        self.put(base + 0x2ac00, "Q", base + 0x1f94c)
        self.put(base + 0x2ac08, "Q", base + 0x1f950)
        self.put(base + 0x2a7e0, "Q", base + 0x1f974)
        self.put(base + 0x2a7e8, "Q", base + 0x1f978)
        self.queue_task_return = 0x20001010
        self.queue_runs = []
        self.pen_data_calls = []
        self.pending_pen_data_returns = {}
        self.put(data + 9, "B", int(case.get("fixed_enabled", False)))
        self.put(data + 12, "f", case.get("fixed_width", 0.0))
        event, rect = self.alloc(64), self.alloc(32)

        synthetic_state = None
        if self.direct_end_pen_probe:
            # Explicit synthetic drawable state for a degenerate terminal
            # path. This is not a constructor-default/native user capture.
            self.put(drawable + 80, "B", 0)
            self.put(drawable + 88, "ff", 0.0, 0.0)
            self.put(drawable + 96, "ff", 0.0, 0.0)
            self.put(drawable + 104, "ff", 0.0, 0.0)
            self.put(drawable + 116, "ff", 0.0, 0.0)
            self.put(drawable + 272, "B", 1)
            self.put(drawable + 276, "ff", 1.0, 0.0)
            self.put(drawable + 284, "f", 0.0)
            synthetic_state = self.drawable_state(drawable)

        def redraw_and_capture():
            try:
                entry = END_PEN if self.direct_end_pen_probe else REDRAW
                self.call(entry, [drawable, event, rect], instruction_limit=2_000_000)
            except Exception:
                from unicorn import arm64_const as R
                print("native failure PC", hex(self.u.reg_read(R.UC_ARM64_REG_PC)), file=sys.stderr)
                print("recent native imports", self.recent_native_imports, file=sys.stderr)
                raise
            rtv1 = self.get(drawable + 16, "Q")[0]
            if self.execute_move_pen:
                # Render-phase probe has run the native queued update, which
                # clears +200..+216 after transferring them. Preserve the GL
                # evidence and skip the saved-mesh-only reader below.
                return {"native_calls": self.gl_calls.copy(),
                        "queue_tasks": [{"vtable_words": q["vtable_words"]}
                                        for q in self.queue_runs]}
            if self.active_add_index is not None:
                self.add_calls[self.active_add_index]["build_calls"] = len(self.build_calls) - self.add_calls[self.active_add_index]["build_start"]
                self.add_calls[self.active_add_index].pop("build_start")
                self.active_add_index = None
            if self.direct_extent_probe or self.zero_length_probe:
                return {"add_points": self.add_calls.copy(), "build_calls": self.build_calls.copy(),
                        "drawable_states": self.drawable_states.copy(),
                        "position_queries": self.position_queries.copy(),
                        "length_queries": self.length_queries.copy()}
            if self.direct_end_pen_probe:
                return {"add_points": self.add_calls.copy(),
                        "build_calls": self.build_calls.copy(),
                        "drawable_states": self.drawable_states.copy(),
                        "position_queries": self.position_queries.copy(),
                        "length_queries": self.length_queries.copy()}
            meshes = []
            for offset in (200, 208, 216):
                vector_address = self.get(rtv1 + offset, "Q")[0]
                vector = self.get(vector_address, "QQQ")
                begin, end, capacity = vector
                if (begin == 0 or capacity == 0 or not (begin <= end <= capacity)
                        or (end - begin) % 16 or end - begin > 4 * 1024 * 1024):
                    raise RuntimeError(f"native output vector has invalid bounds at {offset}: {vector!r}, vector={vector_address:#x}")
                values = self.get(begin, "f" * ((end - begin) // 4)) if end > begin else ()
                meshes.append({"buffer": f"mesh_{offset}",
                               "vertex_f32_bits": [struct.pack("<f", value).hex() for value in values],
                               "vertex_count": (end - begin) // 16})
            return {"orientation_read_indices": self.orientation_reads.copy(),
                    "drawable_states": self.drawable_states.copy(),
                    "position_queries": self.position_queries.copy(),
                    "length_queries": self.length_queries.copy(),
                    "angle_bits": self.get(drawable + 284, "I")[0].to_bytes(4, "little").hex(),
                    "direction_bits": [self.get(drawable + 276 + 4 * i, "I")[0].to_bytes(4, "little").hex() for i in range(2)],
                    "meshes": meshes, "add_points": self.add_calls.copy(),
                    "sample_ends": self.sample_ends.copy(), "terminal_start": self.terminal_start,
                    "build_calls": self.build_calls.copy()}

        first = redraw_and_capture()
        for index, query in enumerate(self.position_queries):
            next_add_start = (self.position_queries[index + 1]["add_start"]
                              if index + 1 < len(self.position_queries)
                              else len(self.add_calls))
            query["add_points_after"] = next_add_start - query["add_start"]
            query["add_point_start"] = query.pop("add_start")
        result = {"fresh": first}
        if synthetic_state is not None:
            result["synthetic_state"] = synthetic_state
        if "reuse_history" in case:
            # Reuse this same native drawable (no constructor between callbacks).
            self.history = [tuple(row) for row in case["reuse_history"]]
            last = self.history[-1]
            self.event_end = {"x": last[0], "y": last[1], "orientation": last[2],
                              "pressure": case.get("pressure", 0.5), "tilt": case.get("tilt", 0.0),
                              "time": case.get("time", 101), "tool": case.get("tool", 2)}
            self.add_calls, self.build_calls, self.orientation_reads = [], [], []
            self.sample_ends, self.terminal_start = [], None
            self.drawable_states, self.position_queries, self.length_queries = [], [], []
            self.pending_position_returns, self.pending_length_returns = {}, {}
            result["reused"] = redraw_and_capture()
        return result

def f32bits(values):
    return [struct.pack("<f", value).hex() for value in values]


def capture_one(color, history=None, size=6.0, terminal_event=False,
                saved_wrapper_case=None):
    history = history or [[0.0, 0.0, 0.0], [4.0, 1.0, 0.35], [9.0, 5.0, 0.7]]
    full_history = [list(row) for row in history]
    case_history = full_history[:-1] if terminal_event else full_history
    case = {"id": f"saved_draw_{color:08x}", "history": case_history,
            "size": size, "color": color}
    if terminal_event:
        case["end"] = full_history[-1]
    native = NativeDrawCalligraphy(LIBDIR, OBJDUMP)
    native.execute_move_pen = True
    update_entries = []
    draw_entries = []
    rt_slots = (224, 232, 240)
    graphics_slots = (248, 256, 264)

    from unicorn import UC_HOOK_CODE, arm64_const as R

    def on_native_code(u, address, size, _data):
        if address not in (native.syms[UPDATE_BUFFER], GRAPHICS_BASE + 0x52464):
            return
        drawable = getattr(native, "drawable", 0)
        rt = native.get(drawable + 16, "Q")[0] if drawable else 0
        vector_begin_to_offset = {}
        object_to_slot = {}
        if rt:
            for offset in rt_slots:
                vector = native.get(rt + offset, "Q")[0]
                vector_begin = native.get(vector, "Q")[0]
                vector_begin_to_offset[vector_begin] = offset
            for offset in graphics_slots:
                object_to_slot[native.get(rt + offset, "Q")[0]] = offset
        if address == native.syms[UPDATE_BUFFER]:
            obj = u.reg_read(R.UC_ARM64_REG_X0)
            data = u.reg_read(R.UC_ARM64_REG_X2)
            count = u.reg_read(R.UC_ARM64_REG_X3)
            update_entries.append({"graphics_slot": object_to_slot.get(obj),
                                   "vector_slot": vector_begin_to_offset.get(data),
                                   "vertex_count": count})
        if address == GRAPHICS_BASE + 0x52464:
            obj = u.reg_read(R.UC_ARM64_REG_X0)
            draw_entries.append({"graphics_slot": object_to_slot.get(obj),
                                 "native_args": [u.reg_read(R.UC_ARM64_REG_X1),
                                                 u.reg_read(R.UC_ARM64_REG_X2),
                                                 u.reg_read(R.UC_ARM64_REG_X3)]})

    native.u.hook_add(UC_HOOK_CODE, on_native_code)
    with contextlib.redirect_stdout(io.StringIO()):
        run = native.run(case)
    if native.u.reg_read(fountain_native.R.UC_ARM64_REG_PC) != 0x200F1000:
        raise AssertionError("saved RedrawPen/queued update did not return to NativeFountain sentinel")

    rt = native.get(native.drawable + 16, "Q")[0]
    vector_data = {}
    for offset in rt_slots:
        vector = native.get(rt + offset, "Q")[0]
        begin, end, capacity = native.get(vector, "QQQ")
        if not begin or not begin <= end <= capacity or (end - begin) % 16:
            raise AssertionError(f"invalid native vector bounds at RTV1+{offset}")
        payload = bytes(native.u.mem_read(begin, end - begin))
        vector_data[offset] = {"payload": payload,
                               "vertices": len(payload) // 16,
                               "sha256": hashlib.sha256(payload).hexdigest()}

    uploads = [entry["buffer_data"] for entry in native.gl_calls
               if entry["name"] == "glBufferData"
               and entry.get("buffer_data", {}).get("size", 0) > 0]
    if len(uploads) != 3:
        raise AssertionError(f"expected three nonempty native queued uploads, got {len(uploads)}")
    upload_records = []
    for upload in uploads:
        payload = bytes.fromhex(upload["bytes_hex"])
        matches = [offset for offset, vector in vector_data.items()
                   if vector["payload"] == payload]
        if len(matches) != 1:
            raise AssertionError("queued upload did not exactly match one native RTV1 vector")
        source_vector = vector_data[matches[0]]
        if upload["size"] != len(payload) or len(payload) != source_vector["vertices"] * 16:
            raise AssertionError("queued upload size does not match native Vector4<f32> payload")
        payload_sha256 = hashlib.sha256(payload).hexdigest()
        if payload_sha256 != source_vector["sha256"]:
            raise AssertionError("queued upload hash does not match its native RTV1 source vector")
        upload_records.append({"source_vector_slot": matches[0],
                               "byte_count": upload["size"],
                               "vertex_count": upload["size"] // 16,
                               "sha256": payload_sha256,
                               "vertex_format": "Vector4<f32> little-endian",
                               "bytes_hex": payload.hex()})

    # Actual subbitmap pixels/transform belong to the host canvas. Supply only
    # an explicit identity projection and no-op bind/unbind callbacks.
    bitmap_vtable_base = 0x20002000
    bitmap = native.alloc(64)
    bitmap_vtable = native.alloc(128)
    matrix = native.alloc(64)
    native.put(bitmap, "Q", bitmap_vtable)
    native.put(matrix, "16f", 1.0, 0.0, 0.0, 0.0,
               0.0, 1.0, 0.0, 0.0,
               0.0, 0.0, 1.0, 0.0,
               0.0, 0.0, 0.0, 1.0)
    bitmap_methods = {2: bitmap_vtable_base,
                      5: bitmap_vtable_base + 0x10,
                      6: bitmap_vtable_base + 0x20}
    for index, address in bitmap_methods.items():
        native.put(bitmap_vtable + index * 8, "Q", address)
    bitmap_calls = {"matrix": 0, "bind": 0, "unbind": 0}

    def on_bitmap(u, address, size, _data):
        if address not in bitmap_methods.values():
            return
        kind = {bitmap_vtable_base: "matrix",
                bitmap_vtable_base + 0x10: "bind",
                bitmap_vtable_base + 0x20: "unbind"}[address]
        bitmap_calls[kind] += 1
        if kind == "matrix":
            out = u.reg_read(R.UC_ARM64_REG_SP) + 8
            native.put(out, "16f", 1.0, 0.0, 0.0, 0.0,
                       0.0, 1.0, 0.0, 0.0,
                       0.0, 0.0, 1.0, 0.0,
                       0.0, 0.0, 0.0, 1.0)
        u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))

    native.u.hook_add(UC_HOOK_CODE, on_bitmap)
    draw_gl_start = len(native.gl_calls)
    native.call(DRAW, [rt, bitmap, matrix], instruction_limit=500_000)
    sentinel_pc = native.u.reg_read(R.UC_ARM64_REG_PC)
    if sentinel_pc != 0x200F1000:
        raise AssertionError(f"native RTV1::Draw did not return to sentinel: {sentinel_pc:#x}")
    draw_gl = native.gl_calls[draw_gl_start:]

    if len(update_entries) < 3:
        raise AssertionError("expected three native GraphicsObject::UpdateBuffer entries")
    native_update_records = []
    for entry in update_entries:
        vector = vector_data.get(entry["vector_slot"])
        if not vector or entry["vertex_count"] != vector["vertices"]:
            raise AssertionError(f"native UpdateBuffer did not consume its mapped vector: {entry}")
        native_update_records.append({"graphics_slot": entry["graphics_slot"],
                                      "source_vector_slot": entry["vector_slot"],
                                      "vertex_count": entry["vertex_count"],
                                      "sha256": vector["sha256"]})

    expected_draw_slots = [256, 264, 248]
    expected_vector_for_graphics = {248: 224, 256: 232, 264: 240}
    if [entry["graphics_slot"] for entry in draw_entries] != expected_draw_slots:
        raise AssertionError(f"native GraphicsObject draw order changed: {draw_entries}")
    gl_draws = [entry for entry in draw_gl if entry["name"] == "glDrawArrays"]
    if len(gl_draws) != 3 or len(draw_entries) != 3:
        raise AssertionError("expected three actual native GraphicsObject and GLES draw calls")
    draw_records = []
    for draw, gl_draw in zip(draw_entries, gl_draws):
        vector_slot = expected_vector_for_graphics[draw["graphics_slot"]]
        expected_count = vector_data[vector_slot]["vertices"]
        mode, first, count = gl_draw["draw"][:3]
        if count != expected_count:
            raise AssertionError("native GL draw count differs from source vector")
        draw_records.append({"graphics_slot": draw["graphics_slot"],
                             "source_vector_slot": vector_slot,
                             "mode": mode, "first": first,
                             "vertex_count": count,
                             "source_sha256": vector_data[vector_slot]["sha256"]})

    program_kind = {}
    program_uniforms = {}
    for program, shader_ids in native.gl_program_shaders.items():
        sources = [source for shader in shader_ids
                   for source in native.gl_shader_sources.get(shader, [])]
        source_text = "\n".join(sources)
        if "fragTexcoord" in source_text:
            kind = "calligraphy-core"
        elif "inputColor.xyz" in source_text:
            kind = "calligraphy-fringe"
        else:
            continue
        program_kind[program] = kind
        program_uniforms[kind] = {
            "active": [{"name": name, "gl_type": gl_type}
                       for name, gl_type in native.program_active_uniforms(program)],
            "fragment_output": next((line.strip() for source in sources
                                     for line in source.splitlines()
                                     if "gl_FragColor" in line), None),
            "shader_sha256": [hashlib.sha256(source.encode()).hexdigest()
                              for source in sources]}

    uniform_calls = []
    active_program = None
    for entry in draw_gl:
        if entry["name"] == "glUseProgram":
            active_program = program_kind.get(entry["args"][0])
        elif entry["name"] == "glUniformMatrix4fv":
            uniform_calls.append({"program": active_program,
                                  "uniform": "ProjectionMatrix",
                                  "kind": "mat4",
                                  "matrix_f32_bits": entry["matrix_f32_bits"]})
        elif entry["name"] == "glUniform4fv":
            uniform_calls.append({"program": active_program,
                                  "uniform": "inputColor",
                                  "kind": "vec4",
                                  "value_f32_bits": entry["float_f32_bits"]})
    native_binding_types = []
    for shader_offset, names in ((272, ("ProjectionMatrix", "inputColor")),
                                 (280, ("ProjectionMatrix", "inputColor"))):
        shader = native.get(rt + shader_offset, "Q")[0]
        for field, label in zip((0, 8), names):
            binding = native.get(shader + field, "Q")[0]
            native_binding_types.append({"shader_slot": shader_offset,
                                         "uniform": label,
                                         "binding_type": native.get(binding + 8, "I")[0]})

    color_call = next(entry for entry in native.pen_data_calls
                      if entry["argb"] == color)
    uniform_color_bits = next(entry["value_f32_bits"] for entry in uniform_calls
                              if entry["uniform"] == "inputColor")
    expected_color = [(color >> shift) & 0xFF for shift in (16, 8, 0, 24)]
    expected_color_bits = f32bits([component / 255.0 for component in expected_color])
    if uniform_color_bits != expected_color_bits:
        raise AssertionError("native SetPenData floats did not reach native vec4 binding")

    blend = []
    for entry in draw_gl:
        if entry["name"] == "glBlendEquationSeparate":
            blend.append({"call": entry["name"], "values": entry["values"][:2]})
        elif entry["name"] == "glBlendFuncSeparate":
            blend.append({"call": entry["name"], "values": entry["values"][:4]})
        elif entry["name"] in ("glEnable", "glDisable"):
            blend.append({"call": entry["name"], "values": entry["args"][:1]})
        elif entry["name"] == "glColorMask":
            blend.append({"call": entry["name"], "values": entry["args"][:4]})

    return {"input": {"history_xy_orientation": full_history,
                      "pen_size": case["size"],
                      "argb": f"0x{color:08x}",
                      **({"saved_wrapper_case": saved_wrapper_case} if saved_wrapper_case else {})},
            "saved_redraw": {"native_add_point_count": len(native.add_calls),
                             "native_build_mesh_count": len(native.build_calls),
                             "orientation_reads": len(native.orientation_reads),
                             **({"native_add_points": [
                                 {"vector_bits": entry["vector_bits"],
                                  "extent_bits": entry["extent_bits"],
                                  "pre_state": entry["pre_state"],
                                  "build_calls": entry.get("build_calls", 0)}
                                 for entry in native.add_calls
                             ],
                                "native_build_rectangles": native.build_calls}
                                if saved_wrapper_case else {})},
            "set_pen_data": {"native_argb": f"0x{color_call['argb']:08x}",
                             "size_f32_bits": color_call["size_bits"],
                             "rgba_f32_bits": f32bits(native.get(rt + 44, "4f"))},
            "queued_update": {"worker_runs": len(native.queue_runs),
                              "update_buffer_entries": native_update_records,
                              "uploads": sorted(upload_records, key=lambda item: item["source_vector_slot"])},
            "draw": {"entry_return_pc": f"0x{sentinel_pc:08x}",
                     "native_graphics_draw_count": len(draw_entries),
                     "gles_import_count": len(draw_gl),
                     "draws": draw_records,
                     "uniform_bindings": native_binding_types,
                     "active_uniform_reflection": program_uniforms,
                     "uniform_calls": uniform_calls,
                     "blend_calls": blend,
                     "host_subbitmap_callbacks": bitmap_calls},
            "native_run_counts": {"queue_runs": len(native.queue_runs),
                                  "add_points": len(native.add_calls),
                                  "build_meshes": len(native.build_calls),
                                  "orientation_reads": len(native.orientation_reads)}}


def capture():
    colors = (0x80112233, 0x4044AAEE)
    results = [capture_one(color) for color in colors]
    wrapper_fixture = json.loads((ROOT / "conformance/calligraphy-saved-wrapper-native.json").read_text())
    wrapper = next(case for case in wrapper_fixture["cases"] if case["id"] == "three-samples")
    wrapper_history = [
        [row[0], row[1], wrapper["orientation"][index]]
        for index, row in enumerate(wrapper["rows"])
    ]
    results.append(capture_one(0x80112233, wrapper_history, 6.0,
                               terminal_event=True, saved_wrapper_case=wrapper["id"]))
    return {"schema": "sdocx-calligraphy-v1-native-queued-draw-host-gles-v1",
            "evidence": {"calligraphy_library_sha256": LIB_SHA,
                         "renderer_library_sha256": fountain_native.HASHES["Renderer"],
                         "graphics_library_sha256": fountain_native.HASHES["Graphics"],
                         "host_context": "Pinned saved RedrawPen/endPen execute through actual WorkerTask::Member2 queue Run, RTV1::Update, and native GraphicsObject::UpdateBuffer. Native RTV1::Draw and Renderer GLES GraphicsObject, BlendStateObject, OpenGLShaderProgram, and ParameterBindingImpl methods execute. Host supplies explicit MotionEvent values, an identity-transform ISPSubBitmap callback interface, synthetic EGL context/GLES object handles/status, source-derived uniform reflection, and intercepted GLES imports. After queued Update, host code redirects PC at movePen+0x3b4 to the native epilogue (+0x3e4); at redraw+0x1f5b4 the queue callback returns through a host trampoline. These redirects skip remaining canvas-size/density metadata; they are not native early returns. Draw then uses the resulting RTV1 state.",
                         "limits": ["No Android EGL/GLES driver or device pixels", "Shader compile/link success, uniform reflection and locations are host-adapter values; type is derived from pinned captured GLSL", "Identity subbitmap transform and bind/unbind are synthetic host inputs", "GL import calls are native call arguments, not executed driver state"]},
            "cases": results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--fixture", type=Path,
                        default=ROOT / "conformance/calligraphy-draw-native.json")
    args = parser.parse_args()
    output = capture()
    encoded = json.dumps(output, indent=2) + "\n"
    if args.write_fixture:
        args.fixture.write_text(encoded)
    elif json.loads(args.fixture.read_text()) != output:
        raise SystemExit("native queued Draw output differs from fixture")
    print(f"{len(output['cases'])} native queued Draw cases; fixture {args.fixture}")


if __name__ == "__main__":
    main()
