#!/usr/bin/env python3
"""Join native Pencil3 V3 particle streams to the pinned host GLES shader.

This comparison replays captured native Model/Drawing/Redraw/AddPoint streams
from the pinned sampler fixture through extracted Pencil3 GLSL and masks on
host Mesa. The separate sampler oracle produced the native captures; this
runner does not execute Model/Drawing, native V3 Draw, Android GLES, or the app
target lifecycle.
"""
import argparse
import base64
import ctypes as C
import hashlib
import json
import math
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "conformance"))
from inkpen_paint_native import GLES, cfunc, read_va

LIB = ROOT / "scratch/apk-analysis-native/arm64-v8a/libSPenPencil3.so"
INPUT = ROOT / "conformance/pencil3-sampler-inputs-native.json"
TEXTURE_FIXTURE = ROOT / "conformance/pencil-textures-v1.json"
PAINT_FIXTURE = ROOT / "conformance/pencil-paint-v1.json"
RUNNER = Path(__file__).resolve()
OUT = RUNNER.with_name("pencil-v3-paint-join.json")

PENCIL3_SHA256 = "41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359"
INPUT_SHA256 = "ddeb53cff680d0a01b6790a67fb9c667921c10edf4ac1d009a1a9933adc3455b"
SAMPLER_RUNNER_SHA256 = "c8944bf164bc6bf7a88a17f72b8bd1c2a00c4c33679d9fd4e09dd0d06c2a2429"
TEXTURE_FIXTURE_SHA256 = "2e48cf1a63ff6444761ca7392de8e97fd6f957c7fa4eeb47d8d8fa1e00884918"
PAINT_FIXTURE_SHA256 = "1b533214ebcf4130784052d76328bfae8dec5af480134ac6115257117a5fcc1b"
PAINT_RUNNER_SHA256 = "392200a1cde381d2dd85d60d73ead7b64722e0ac5102e51a47bffc1b22dc31eb"
GLES_HELPER_SHA256 = "a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae"
WIDTH = HEIGHT = 128
VERTEX, FRAGMENT, QUAD = 0xF9730, 0xF9948, 0xF9690
SCALES = (1.0, 1.5, 2.0)
INPUT_COLOR = (0.18, 0.32, 0.72, 0.8)  # synthetic; native color handoff is outside this join


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_pixels(gles):
    out = C.create_string_buffer(WIDTH * HEIGHT * 4)
    gles.f["glPixelStorei"](0x0D05, 1)
    gles.f["glReadPixels"](0, 0, WIDTH, HEIGHT, 0x1908, 0x1401, out)
    return out.raw


def upload_texture(gles, unit, width, height, rgba, min_filter, mag_filter, wrap_s, wrap_t):
    f = gles.f
    handle = C.c_uint()
    f["glGenTextures"](1, C.byref(handle))
    f["glActiveTexture"](0x84C0 + unit)
    f["glBindTexture"](0x0DE1, handle.value)
    for pname, value in ((0x2801, min_filter), (0x2800, mag_filter),
                         (0x2802, wrap_s), (0x2803, wrap_t)):
        f["glTexParameteri"](0x0DE1, pname, value)
    buf = C.create_string_buffer(rgba)
    f["glPixelStorei"](0x0CF5, 1)
    f["glTexImage2D"](0x0DE1, 0, 0x8058, width, height, 0, 0x1908, 0x1401,
                      C.cast(buf, C.c_void_p))
    return handle.value


def stream_from_phase(case_name, route):
    drawing = route["phases"]["draw_object_stroke"]
    saved = route["phases"]["saved_redraw"]
    words = drawing["native_packed_stream_f32_bits_le"]
    stream = b"".join(bytes.fromhex(word) for word in words)
    if sha(stream) != drawing["native_packed_stream_sha256"]:
        raise ValueError(f"{case_name}: Drawing stream SHA mismatch")
    if len(words) != drawing["native_addpoint_callback_count"] * 5:
        raise ValueError(f"{case_name}: Drawing stream does not contain five words per callback")
    if not saved["native_packed_stream_same_bytes_as_drawing"]:
        raise ValueError(f"{case_name}: saved Redraw stream is not proven byte-identical")
    if not saved["native_rt_identity_same_as_drawing"]:
        raise ValueError(f"{case_name}: native Draw and saved redraw RT identity differs")
    if saved["native_packed_stream_sha256"] != sha(stream):
        raise ValueError(f"{case_name}: saved stream SHA differs from Draw stream")
    if saved["native_addpoint_callback_count"] != drawing["native_addpoint_callback_count"]:
        raise ValueError(f"{case_name}: Draw/saved callback counts differ")
    floats = [struct.unpack("<f", bytes.fromhex(word))[0] for word in words]
    if not all(math.isfinite(value) for value in floats):
        raise ValueError(f"{case_name}: non-finite native stream value")
    records = [floats[i:i + 5] for i in range(0, len(floats), 5)]
    return stream, records, drawing, saved


def setup_host(lib_bytes, atlas_rgba, paper_rgba, paper_white):
    g = GLES()
    f = g.f
    source = lambda va: read_va(lib_bytes, va, 8192).split(b"\0", 1)[0].decode()
    program = g.program(source(VERTEX), source(FRAGMENT))
    f["glUseProgram"](program)
    brush = upload_texture(g, 0, 120, 1920, atlas_rgba,
                           0x2600, 0x2600, 0x812F, 0x812F)
    paper_bytes = bytes((255, 0, 0, 0)) * (150 * 150) if paper_white else paper_rgba
    paper = upload_texture(g, 1, 150, 150, paper_bytes,
                           0x2600, 0x2600, 0x2901, 0x2901)
    f["glUniform1i"](f["glGetUniformLocation"](program, b"pointPattern"), 0)
    f["glUniform1i"](f["glGetUniformLocation"](program, b"paperTexture"), 1)
    f["glUniform4f"](f["glGetUniformLocation"](program, b"inputColor"), *INPUT_COLOR)
    f["glUniform2f"] = cfunc(g.gl, "glUniform2f", None, [C.c_int, C.c_float, C.c_float])
    f["glUniform2f"](f["glGetUniformLocation"](program, b"uPaperScale"), 1.0, 1.0)

    quad = [struct.unpack("<4f", read_va(lib_bytes, QUAD + i * 16, 16)) for i in range(4)]
    vao = C.c_uint()
    f["glGenVertexArrays"](1, C.byref(vao))
    f["glBindVertexArray"](vao.value)
    g.buffer(0x8892, [value for vertex in quad for value in vertex])
    f["glEnableVertexAttribArray"](0)
    f["glVertexAttribPointer"](0, 4, 0x1406, 0, 0, None)
    f["glVertexAttribDivisor"](0, 0)

    target = C.c_uint()
    f["glGenTextures"](1, C.byref(target))
    f["glBindTexture"](0x0DE1, target.value)
    for pname, value in ((0x2801, 0x2600), (0x2800, 0x2600),
                         (0x2802, 0x812F), (0x2803, 0x812F)):
        f["glTexParameteri"](0x0DE1, pname, value)
    f["glTexImage2D"](0x0DE1, 0, 0x8058, WIDTH, HEIGHT, 0, 0x1908, 0x1401, None)
    framebuffer = C.c_uint()
    f["glGenFramebuffers"](1, C.byref(framebuffer))
    f["glBindFramebuffer"](0x8D40, framebuffer.value)
    f["glFramebufferTexture2D"](0x8D40, 0x8CE0, 0x0DE1, target.value, 0)
    status = f["glCheckFramebufferStatus"](0x8D40)
    if status != 0x8CD5:
        raise RuntimeError(f"RGBA8 output target is incomplete: {status:#x}")
    f["glViewport"](0, 0, WIDTH, HEIGHT)
    f["glActiveTexture"](0x84C0)
    f["glBindTexture"](0x0DE1, brush)
    f["glActiveTexture"](0x84C1)
    f["glBindTexture"](0x0DE1, paper)
    f["glEnable"](0x0BE2)
    f["glBlendEquationSeparate"](0x8006, 0x8006)
    f["glBlendFuncSeparate"](0x0302, 0x0303, 0x0001, 0x0303)
    return g, program, quad, paper_bytes


def draw(g, program, records, scale):
    f = g.f
    f["glUseProgram"](program)
    projection = (C.c_float * 16)(2.0 * scale / WIDTH, 0, 0, 0,
                                  0, 2.0 * scale / HEIGHT, 0, 0,
                                  0, 0, 1, 0, 0, 0, 0, 1)
    f["glUniformMatrix4fv"](f["glGetUniformLocation"](program, b"uProjectionMatrix"),
                            1, 0, projection)
    f["glClearColor"](0, 0, 0, 0)
    f["glClear"](0x4000)
    packed_values = [value for record in records for value in record]
    if records:
        g.buffer(0x8892, packed_values)
        f["glEnableVertexAttribArray"](1)
        f["glVertexAttribPointer"](1, 4, 0x1406, 0, 20, None)
        f["glVertexAttribDivisor"](1, 1)
        f["glEnableVertexAttribArray"](2)
        f["glVertexAttribPointer"](2, 1, 0x1406, 0, 20, C.c_void_p(16))
        f["glVertexAttribDivisor"](2, 1)
        f["glDrawArraysInstanced"](0x0005, 0, 4, len(records))
    pixels = read_pixels(g)
    error = f["glGetError"]()
    if error:
        raise RuntimeError(f"GLES error after {len(records)} particles at scale {scale}: {error:#x}")
    offsets = [i for i in range(3, len(pixels), 4) if pixels[i]]
    spot = None
    if offsets:
        alpha_offset = offsets[len(offsets) // 2]
        pixel_offset = alpha_offset - 3
        rgba = list(pixels[pixel_offset:pixel_offset + 4])
        expected = [round(channel * rgba[3]) for channel in INPUT_COLOR[:3]]
        errors = [abs(rgba[i] - expected[i]) for i in range(3)]
        if max(errors) > 2:
            raise AssertionError(f"source-over spot RGB differs: got {rgba}, expected {expected}")
        spot = {"pixel_xy_bottom_left": [(pixel_offset // 4) % WIDTH, (pixel_offset // 4) // WIDTH],
                "rgba8": rgba, "expected_rgb8_from_alpha": expected,
                "max_rgb_error": max(errors)}
    return pixels, spot, sum(pixels[i] != 0 for i in range(3, len(pixels), 4))


def make_fixture():
    lib_bytes = LIB.read_bytes()
    if sha(lib_bytes) != PENCIL3_SHA256:
        raise ValueError("Pencil3 DSO differs from pinned Samsung Notes 4.4.45.37")
    input_bytes = INPUT.read_bytes()
    if sha(input_bytes) != INPUT_SHA256:
        raise ValueError("native sampler fixture differs from pinned V3 capture")
    input_doc = json.loads(input_bytes)
    atlas_u32 = read_va(lib_bytes, 0x12C8C, 16 * 120 * 120 * 4)
    atlas_rgba = b"".join(bytes((v, 0, 0, 0)) for v in atlas_u32[::4])
    paper_u8 = read_va(lib_bytes, 0xF3C8C, 150 * 150)
    paper_rgba = b"".join(bytes((v, 0, 0, 0)) for v in paper_u8)

    base_paint = json.loads(PAINT_FIXTURE.read_text())
    shader_sources = [read_va(lib_bytes, va, 8192).split(b"\0", 1)[0]
                      for va in (VERTEX, FRAGMENT)]
    if sha(atlas_rgba) != base_paint["atlas_sha256"]:
        raise ValueError("extracted atlas differs from pinned host paint fixture")
    if sha(paper_rgba) != base_paint["paper_sha256"]:
        raise ValueError("extracted paper differs from pinned host paint fixture")
    if [sha(source) for source in shader_sources] != base_paint["shader_sha256"]:
        raise ValueError("extracted shader differs from pinned host paint fixture")

    if sha(TEXTURE_FIXTURE.read_bytes()) != TEXTURE_FIXTURE_SHA256:
        raise ValueError("pinned Pencil texture transport fixture changed")
    if sha(PAINT_FIXTURE.read_bytes()) != PAINT_FIXTURE_SHA256:
        raise ValueError("pinned base Pencil host paint fixture changed")
    if sha((ROOT / "conformance/pencil_paint_native.py").read_bytes()) != PAINT_RUNNER_SHA256:
        raise ValueError("pinned base Pencil host paint runner changed")
    if sha((ROOT / "conformance/inkpen_paint_native.py").read_bytes()) != GLES_HELPER_SHA256:
        raise ValueError("pinned host GLES helper changed")
    if input_doc["runner_sha256"] != SAMPLER_RUNNER_SHA256:
        raise ValueError("native V3 sampler runner provenance differs")
    if input_doc["schema"] != "pencil3-sampler-inputs-native-v1":
        raise ValueError("unexpected native sampler fixture schema")

    cases_out = {}
    gles_identity = None
    for name, case in sorted(input_doc["cases"].items()):
        route = case["routes"]["v3"]
        if route["selected_vtable_id"] != "v3" or route["advanced_setting_state"] != "empty":
            raise ValueError(f"{name}: not a captured V3 empty-setting route")
        if not route["native_source_arrays_verified"]:
            raise ValueError(f"{name}: source-array verification missing")
        stream, records, drawing_phase, saved_phase = stream_from_phase(name, route)
        profile = case["input_profile"]
        profile_row = {
            "input_profile": profile,
            "native_selected_drawable": route["selected_drawable_class"],
            "native_source_array_sha256": route["native_source_array_sha256"],
            "native_particle_count": len(records),
            "native_stream_sha256": sha(stream),
            "native_attribute_order": ["center_x", "center_y", "half_size", "signed_pattern_row", "point_alpha"],
            "native_attribute_ranges": {
                "center_x": [min(r[0] for r in records), max(r[0] for r in records)],
                "center_y": [min(r[1] for r in records), max(r[1] for r in records)],
                "half_size": [min(r[2] for r in records), max(r[2] for r in records)],
                "pattern_row_unique": sorted({int(r[3]) for r in records}),
                "point_alpha": [min(r[4] for r in records), max(r[4] for r in records)],
            },
            "native_phases": {
                "draw_object_stroke": {
                    "particle_count": drawing_phase["native_addpoint_callback_count"],
                    "packed_stream_sha256": drawing_phase["native_packed_stream_sha256"],
                },
                "saved_redraw": {
                    "particle_count": saved_phase["native_addpoint_callback_count"],
                    "packed_stream_sha256": saved_phase["native_packed_stream_sha256"],
                    "same_bytes_as_drawing": saved_phase["native_packed_stream_same_bytes_as_drawing"],
                    "same_native_rt": saved_phase["native_rt_identity_same_as_drawing"],
                    "host_pixels_reuse_draw_stream_after_native_byte_equality_assertion": True,
                },
            },
        }

        outputs = {}
        for paper_name, paper_white in (("embedded_default_paper", False), ("white_paper_control", True)):
            g, program, quad, actual_paper_rgba = setup_host(lib_bytes, atlas_rgba, paper_rgba, paper_white)
            gles_identity = g.identity
            scale_rows = {}
            for scale in SCALES:
                pixels, spot, nonzero = draw(g, program, records, scale)
                scale_rows[str(scale)] = {
                    "rgba8_sha256": sha(pixels),
                    "rgba8_bytes": len(pixels),
                    "rgba8_zlib_base64": base64.b64encode(zlib.compress(pixels, 9)).decode("ascii"),
                    "nonzero_alpha_pixel_count": nonzero,
                    "analytic_source_over_spotcheck": spot,
                }
            outputs[paper_name] = {
                "paper_red_rgba_sha256": sha(actual_paper_rgba),
                "scale_outputs": scale_rows,
            }
        profile_row["host_paint_replay"] = outputs
        cases_out[name] = profile_row

    return {
        "schema": "pencil3-v3-pinned-shader-host-join-v1",
        "scope": "Captured native Model/Drawing V3 source arrays, saved redraw sampler, private RNG and RTV1 AddPoint packed streams from the pinned five-case sampler fixture are replayed as instanced attributes through the pinned Pencil3 embedded shader and mask bytes on host Mesa GLES. Saved phase reuses the draw stream only after native fixture's full-byte-equality and same-RT assertions. This runner does not re-execute the native sampler, Model or Drawing; no native V3 Draw, native FBO, Android GLES, document PDF, or production Renderer is executed.",
        "provenance": {
            "pencil3_dso_sha256": sha(lib_bytes),
            "native_sampler_fixture_sha256": sha(input_bytes),
            "native_sampler_runner_sha256": input_doc["runner_sha256"],
            "native_sampler_helper_sha256": input_doc["provenance"]["native_helper_sha256"],
            "texture_transport_fixture_sha256": sha(TEXTURE_FIXTURE.read_bytes()),
            "base_v1_host_paint_fixture_sha256": sha(PAINT_FIXTURE.read_bytes()),
            "base_v1_host_paint_runner_sha256": PAINT_RUNNER_SHA256,
            "host_helper_sha256": sha((ROOT / "conformance/inkpen_paint_native.py").read_bytes()),
            "runner_sha256": sha(RUNNER.read_bytes()),
            "shader_addresses": [hex(VERTEX), hex(FRAGMENT)],
            "quad_address": hex(QUAD),
            "atlas_source_va": hex(0x12C8C),
            "paper_source_va": hex(0xF3C8C),
        },
        "host": gles_identity,
        "host_assumptions": {
            "target_rgba8": [WIDTH, HEIGHT],
            "target_origin": "bottom-left in stored GL readback bytes",
            "projection": "synthetic centered projection; logical units per pixel = 1/scale; matrix diagonal is 2*scale/128",
            "scales_pixels_per_logical_unit": list(SCALES),
            "paper_scale": [1.0, 1.0],
            "paper_coordinate_phase": "embedded shader vCanvasUV=(clip.xy/clip.w+1)/2 on the synthetic centered target; paper samples vCanvasUV*uPaperScale with repeat",
            "input_color": list(INPUT_COLOR),
            "blend": ["FUNC_ADD", "SRC_ALPHA", "ONE_MINUS_SRC_ALPHA", "ONE", "ONE_MINUS_SRC_ALPHA"],
            "brush_sampling": {"filter": "NEAREST", "wrap_s": "CLAMP_TO_EDGE", "wrap_t": "CLAMP_TO_EDGE"},
            "paper_sampling": {"filter": "NEAREST", "wrap_s": "REPEAT", "wrap_t": "REPEAT"},
            "white_paper_control": "A 150x150 red=255 texture is substituted; all geometry, shader, projection and blend settings remain the same.",
        },
        "boundary": {
            "native_geometry_source": "pencil3-sampler-inputs-native.json; native callback stream words were captured after actual selected GLV3 redraw and actual RTV1 AddPoint callback path",
            "native_phase_join": "Saved redraw stream hash/count and byte equality with Drawing stream plus same RT identity are checked before reusing captured Drawing words for the saved-phase host replay.",
            "host_graphics": "Host GLES executes the extracted shader strings from pinned Pencil3 DSO with masks read from that DSO; GLES context, target, transforms, color, blend and paper phase are host configured.",
            "not_claimed": ["Native V3 Draw or FBO", "Samsung device pixel parity", "native app projection or paper phase", "actual source color handoff", "production renderer or export output", "vector encoding parity"],
        },
        "cases": cases_out,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    result = make_fixture()
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        OUT.write_text(encoded)
        print(f"wrote {OUT.relative_to(ROOT)}")
    elif not OUT.exists() or json.loads(OUT.read_text()) != result:
        raise SystemExit("Pencil3 V3 host paint join differs from frozen fixture")
    print("host GLES:", json.dumps(result["host"], sort_keys=True))
    for name, case in result["cases"].items():
        print(name, case["native_particle_count"], case["native_stream_sha256"])


if __name__ == "__main__":
    main()
