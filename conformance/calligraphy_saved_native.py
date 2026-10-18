"""Bounded ARM64 oracle for MontblancCalligraphyPen V1 saved redraw.

Runs the pinned GLV1 RedrawPen body with an explicit MotionEvent history and
records the native Vector4/extent calls at RTV1::AddPoint. This is geometry
evidence, not GPU pixel evidence.
"""
import hashlib
import json
import math
import struct
import sys
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "conformance"))
import fountain_native
from fountain_native import NativeFountain

LIB_SHA = "75aa5312fba02e21ff01d713187d2fd8d015ac4781e2d58da85ae700114e6f42"
REDRAW = "SPen::MontblancCalligraphyPenStrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)"
CTOR = "SPen::MontblancCalligraphyPenStrokeDrawableGLV1::MontblancCalligraphyPenStrokeDrawableGLV1(SPen::MontblancCalligraphyPenData*, SPen::PenGLDataManager*)"
ADD = "SPen::MontblancCalligraphyPenStrokeDrawableRTV1::AddPoint(SPen::Vector4<float>, float)"
END_PEN = "SPen::MontblancCalligraphyPenStrokeDrawableGLV1::endPen(SPen::MotionEvent const*, SPen::RectF*)"
BUILD = "SPen::MontblancCalligraphyPenStrokeDrawableRTV1::buildMesh(SPen::MontblancCalligraphyPenStrokeDrawableRTV1::Rect const&, std::__ndk1::vector<SPen::Vector4<float>, std::__ndk1::allocator<SPen::Vector4<float> > >*)"


class NativeCalligraphy(NativeFountain):
    def __init__(self, library_dir, objdump):
        fountain_native.HASHES["MontblancCalligraphyPen"] = LIB_SHA
        super().__init__(library_dir, objdump, ("MontblancCalligraphyPen",))
        from unicorn import UC_HOOK_MEM_INVALID
        self.u.hook_add(UC_HOOK_MEM_INVALID, self.bad_memory)
        digest = hashlib.sha256((library_dir / "libSPenMontblancCalligraphyPen.so").read_bytes()).hexdigest()
        if digest != LIB_SHA:
            raise ValueError(f"unverified calligraphy library: {digest}")
        self.add_address = self.syms[ADD]
        self.build_address = self.syms[BUILD]
        self.redraw_address = self.syms[REDRAW]
        self.end_pen_address = self.syms[END_PEN]
        self.redraw_line_address = self.syms["SPen::MontblancCalligraphyPenStrokeDrawableGLV1::redrawLine(float, float, float)"]
        self.position_address = self.syms["SPen::SmPath::getPosTan(float, SPen::SmPoint*, SPen::SmPoint*)"]
        self.length_address = self.syms["SPen::SmPath::getLength()"]
        self.add_calls = []
        self.build_calls = []
        self.orientation_reads = []
        self.history = []
        self.event_end = {}
        self.queue = 0
        self.output_labels = {}
        self.active_add_index = None
        self.direct_extent_probe = False
        self.zero_length_probe = False
        self.direct_end_pen_probe = False
        self.zero_length_applied = False
        self.drawable_states = []
        self.position_queries = []
        self.length_queries = []
        self.pending_position_returns = {}
        self.pending_length_returns = {}
        self.callbacks.update({
            "operator new(unsigned long)": lambda: self.x(0, self.alloc(self.x(0))),
            "operator delete(void*)": lambda: None,
            "SPen::PenGLDataManager::GetMsgQueue()": lambda: self.x(0, self.queue),
            "SPen::MotionEvent::GetHistorySize() const": lambda: self.x(0, len(self.history)),
            "SPen::MotionEvent::GetHistoricalX(int, int) const": lambda: self.d(0, self.history[self.x(1)][0]),
            "SPen::MotionEvent::GetHistoricalY(int, int) const": lambda: self.d(0, self.history[self.x(1)][1]),
            "SPen::MotionEvent::GetHistoricalOrientation(int) const": lambda: self.f(0, self.history[self.x(1)][2]),
            "SPen::MotionEvent::GetToolType(int) const": lambda: self.x(0, self.event_end["tool"]),
            "SPen::MotionEvent::GetEventTime() const": lambda: self.x(0, self.event_end["time"]),
            "SPen::MotionEvent::GetX(int) const": lambda: self.d(0, self.event_end["x"]),
            "SPen::MotionEvent::GetY(int) const": lambda: self.d(0, self.event_end["y"]),
            "SPen::MotionEvent::GetPressure(int) const": lambda: self.f(0, self.event_end["pressure"]),
            "SPen::MotionEvent::GetOrientation(int) const": lambda: self.f(0, self.event_end["orientation"]),
            "SPen::MotionEvent::GetTilt(int) const": lambda: self.f(0, self.event_end["tilt"]),
            "SPen::MotionEvent::GetDownTime() const": lambda: self.x(0, self.event_end["time"]),
            "SPen::MotionEvent::GetButtonState() const": lambda: self.x(0, 0),
            "SPen::MotionEvent::GetSource() const": lambda: self.x(0, 0),
            "SPen::MotionEvent::GetFlags() const": lambda: self.x(0, 0),
            "SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)": lambda: None,
            "SPen::MotionEvent::~MotionEvent()": lambda: None,
            "SPen::RectF::MakeRect(SPen::PointF const*)": self.rect_make,
            "SPen::RectF::Union(SPen::RectF const&)": lambda: None,
            "SPen::RectF::IncreaseRect(float)": lambda: None,
            "SPen::Error::SetError(unsigned long)": lambda: None,
            "sincos": self.host_sincos,
        })

    def bad_memory(self, u, access, address, size, value, user_data):
        from unicorn import arm64_const as R
        print("bad memory", hex(address), "PC", hex(u.reg_read(R.UC_ARM64_REG_PC)), file=sys.stderr)
        return False

    def rect_make(self):
        pass

    def host_sincos(self):
        from unicorn import arm64_const as R
        angle = struct.unpack("<d", struct.pack("<Q", self.u.reg_read(R.UC_ARM64_REG_D0)))[0]
        self.u.mem_write(self.x(0), struct.pack("<d", math.sin(angle)))
        self.u.mem_write(self.x(1), struct.pack("<d", math.cos(angle)))

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
        if address in self.pending_length_returns:
            record = self.pending_length_returns.pop(address)
            record["length_bits"] = struct.pack("<f", self.f(0)).hex()
        if address in self.pending_position_returns:
            record = self.pending_position_returns.pop(address)
            record["ok"] = self.x(0) != 0
        if address == self.redraw_address:
            self.drawable_states.append({"phase": "redraw_entry", **self.drawable_state(self.x(0))})
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
            from unicorn import arm64_const as R
            u.reg_write(R.UC_ARM64_REG_PC, 0x200f1000)
            u.emu_stop()
            return
        if address == 0x20001000:
            self.x(0, 1)
            u.reg_write(__import__("unicorn").arm64_const.UC_ARM64_REG_PC,
                        u.reg_read(__import__("unicorn").arm64_const.UC_ARM64_REG_LR))
            return
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
        drawable = self.alloc(512)
        self.drawable = drawable
        self.call(CTOR, [drawable, data, manager])
        # V1 reads pen size and optional fixed-width state through PenData.
        settings = self.alloc(64)
        self.put(data, "Q", settings)
        self.put(settings, "f", self.current_size)
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
                raise
            rtv1 = self.get(drawable + 16, "Q")[0]
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


def main():
    from pathlib import Path
    library_dir = ROOT / "scratch/apk-analysis-native/arm64-v8a"
    objdump = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump"
    native = NativeCalligraphy(library_dir, objdump)
    cases = [
        {"id": "three_history_samples", "history": [[0.0, 0.0, 0.0], [4.0, 1.0, 0.35], [9.0, 5.0, 0.7]], "reuse_history": [[2.0, 3.0, 0.1], [8.0, 7.0, -0.2], [14.0, 9.0, 0.15]], "size": 6.0},
        {"id": "one_history_initial_end", "history": [[4.0, 5.0, 0.2]], "size": 6.0},
        {"id": "three_history_zero_orientation", "history": [[0.0, 0.0, 0.0], [4.0, 1.0, 0.0], [9.0, 5.0, 0.0]], "size": 6.0},
        {"id": "five_history_phase_boundaries", "history": [[0.0, 0.0, 0.0], [4.0, 1.0, 0.35], [9.0, 5.0, 0.7], [15.0, 8.0, -0.15], [22.0, 11.0, 0.3]], "size": 6.0},
        {"id": "subunit_extent_clamp", "history": [[0.0, 0.0, 0.0], [4.0, 2.0, 0.0], [8.0, 5.0, 0.0]], "size": 1.0},
    ]
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    group.add_argument("--write-direct-extent-fixture", action="store_true")
    group.add_argument("--check-direct-extent-fixture", action="store_true")
    group.add_argument("--write-zero-length-fixture", action="store_true")
    group.add_argument("--check-zero-length-fixture", action="store_true")
    group.add_argument("--write-zero-terminal-fixture", action="store_true")
    group.add_argument("--check-zero-terminal-fixture", action="store_true")
    parser.add_argument("--fixture", type=Path)
    args = parser.parse_args()
    if args.write_zero_terminal_fixture or args.check_zero_terminal_fixture:
        native.direct_end_pen_probe = True
        case = {"id": "synthetic_zero_length_terminal_endpen", "history": [],
                "end": [0.0, 0.0, 0.0], "size": 6.0}
        result = native.run(case)
        run = result["fresh"]
        compact = {"schema": "sdocx-calligraphy-v1-synthetic-zero-terminal-native-v1",
                   "evidence": {"library": "libSPenMontblancCalligraphyPen.so", "sha256": LIB_SHA,
                                "entry": END_PEN,
                                "host_context": "A constructed GLV1 drawable with attached pen settings is called at endPen directly. The harness supplies zero path start/control/midpoint/accepted/endpoint, angle 0, initial=false, and movement_armed=true; MotionEvent getters are explicit host callbacks.",
                                "limitations": ["Synthetic direct method-entry state, not a constructor default or saved user event", "No RTV1 buffers were initialized or drawn", "No GPU rendering or pixels"]},
                   "case": {**case, "synthetic_state": result["synthetic_state"],
                            "native": {"length_queries": run["length_queries"],
                                       "position_queries": run["position_queries"],
                                       "add_points": run["add_points"]}}}
        fixture = args.fixture or ROOT / "conformance/calligraphy-zero-terminal-native.json"
        encoded = json.dumps(compact, indent=2) + "\n"
        if args.write_zero_terminal_fixture:
            fixture.write_text(encoded)
        elif json.loads(fixture.read_text()) != compact:
            raise SystemExit("native zero-terminal output differs from the checked fixture")
        print(f"synthetic zero-length endPen probe; fixture {fixture}")
        return
    if args.write_zero_length_fixture or args.check_zero_length_fixture:
        native.zero_length_probe = True
        case = {"id": "synthetic_zero_length_saved_step",
                "history": [[0.0, 0.0, 0.0], [4.0, 1.0, 0.35], [9.0, 5.0, 0.7]],
                "size": 6.0}
        run = native.run(case)["fresh"]
        zero_length = next(query for query in run["length_queries"]
                           if query["caller_pc"] == "0x301eb1c"
                           and query["length_bits"] == "00000000")
        failed_position = next(query for query in run["position_queries"]
                               if query["caller_pc"] == "0x301ebb8"
                               and query["ok"] is False)
        compact = {"schema": "sdocx-calligraphy-v1-synthetic-zero-length-native-v1",
                   "evidence": {"library": "libSPenMontblancCalligraphyPen.so", "sha256": LIB_SHA,
                                "entry": REDRAW,
                                "host_context": "GLV1 RedrawPen and endPen run with explicit MotionEvent callbacks; a hook at the first redrawLine entry sets S0/S1/S2 and saved start/control/mid points to zero, accepted to (-2, 0), orientation state to 90 degrees, initial=false, movement_armed=true.",
                                "limitations": ["Synthetic drawable/register override at redrawLine, not constructor defaults or a captured user event", "No GPU rendering or pixels"]},
                   "case": {**case,
                            "synthetic_state": {"start": [0.0, 0.0], "control": [0.0, 0.0],
                                                "midpoint": [0.0, 0.0], "endpoint_registers": [0.0, 0.0],
                                                "accepted": [-2.0, 0.0], "angle_degrees": 90.0,
                                                "initial_pending": False, "movement_armed": True},
                            "native": {"redraw_line_length_bits": zero_length["length_bits"],
                                       "get_pos_tan": {key: failed_position[key]
                                                       for key in ("caller_pc", "distance_bits", "ok", "add_points_after")},
                                       "add_points": run["add_points"][
                                           failed_position["add_point_start"]:
                                           failed_position["add_point_start"] + failed_position["add_points_after"]],
                                       "initial_drawable_state": run["drawable_states"][0]}}}
        fixture = args.fixture or ROOT / "conformance/calligraphy-zero-length-native.json"
        encoded = json.dumps(compact, indent=2) + "\n"
        if args.write_zero_length_fixture:
            fixture.write_text(encoded)
        elif json.loads(fixture.read_text()) != compact:
            raise SystemExit("native zero-length output differs from the checked fixture")
        print(f"synthetic zero-length probe; fixture {fixture}")
        return
    if args.write_direct_extent_fixture or args.check_direct_extent_fixture:
        native.direct_extent_probe = True
        direct_case = {"id": "direct_addpoint_extent_half", "history": [[0.0, 0.0, 0.0], [4.0, 1.0, 0.35], [9.0, 5.0, 0.7]], "size": 6.0}
        run = native.run(direct_case)["fresh"]
        compact = {"schema": "sdocx-calligraphy-v1-addpoint-direct-native-v1",
                   "evidence": {"library": "libSPenMontblancCalligraphyPen.so", "sha256": LIB_SHA,
                                "entry": ADD, "host_context": "The pinned RedrawPen supplies a real constructed RTV1 receiver and native nib vector. At the first AddPoint entry the harness explicitly replaces S0 with 0.5; native AddPoint executes through its five Rect arguments, while buildMesh bodies are skipped at their entry points.",
                                "limitations": ["Synthetic AddPoint extent register override, not a naturally selected caller extent", "No vector append or mesh output in this probe"]},
                   "case": {**direct_case, "native": {"extent_bits": run["add_points"][0]["extent_bits"],
                                                        "vector_bits": run["add_points"][0]["vector_bits"],
                                                        "pre_state": run["add_points"][0]["pre_state"],
                                                        "rectangles": run["build_calls"]}}
                  }
        fixture = args.fixture or ROOT / "conformance/calligraphy-addpoint-direct-native.json"
        encoded = json.dumps(compact, indent=2) + "\n"
        if args.write_direct_extent_fixture:
            fixture.write_text(encoded)
        elif json.loads(fixture.read_text()) != compact:
            raise SystemExit("native direct AddPoint output differs from the checked fixture")
        print(f"direct AddPoint extent probe; fixture {fixture}")
        return
    fixture_path = args.fixture or ROOT / "conformance/calligraphy-saved-native.json"
    output = []
    for case in cases:
        output.append({**case, "native": native.run(case)})
    output_json = {"schema": "sdocx-montblanc-calligraphy-v1-saved-native-v2",
               "evidence": {"library": "libSPenMontblancCalligraphyPen.so", "sha256": LIB_SHA,
                            "entry": REDRAW, "native_scope": "GLV1 RedrawPen history and endPen; one case invokes it twice on the same constructed drawable, then stops before movePen",
                            "inputs": "Synthetic MotionEvent history arrays supplied by this harness, not captured from an application runtime",
                            "host_context": "MotionEvent getters return explicit fixture values; MotionEvent constructor/destructor, RectF bounds operations, queue submit, bounded allocation and host math sincos callback are host operations",
                            "limitations": ["No APK Java callback construction", "No movePen lifecycle", "No GPU rendering or pixels"]},
               "cases": output}
    encoded = json.dumps(output_json, indent=2) + "\n"
    if args.write_fixture:
        fixture_path.write_text(encoded)
    elif json.loads(fixture_path.read_text()) != output_json:
        raise SystemExit("native output differs from the checked fixture")
    print(f"{len(output)} cases; fixture {fixture_path}")


if __name__ == "__main__":
    main()
