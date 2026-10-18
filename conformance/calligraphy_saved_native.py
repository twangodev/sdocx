"""Bounded ARM64 oracle for MontblancCalligraphyPen V1 saved redraw.

Runs the pinned GLV1 RedrawPen body with an explicit MotionEvent history and
records the native Vector4/extent calls at RTV1::AddPoint. This is geometry
evidence, not GPU pixel evidence.
"""
import hashlib
import json
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
        self.add_calls = []
        self.orientation_reads = []
        self.history = []
        self.event_end = {}
        self.queue = 0
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
        })

    def bad_memory(self, u, access, address, size, value, user_data):
        from unicorn import arm64_const as R
        print("bad memory", hex(address), "PC", hex(u.reg_read(R.UC_ARM64_REG_PC)), file=sys.stderr)
        return False

    def rect_make(self):
        pass

    def hook(self, u, address, size, data):
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
            vector = self.x(1)
            self.add_calls.append({"vector_bits": [self.get(vector + 4 * i, "I")[0].to_bytes(4, "little").hex() for i in range(4)],
                                   "extent_bits": struct.pack("<f", self.f(0)).hex()})
        if address in self.plt and self.plt[address] == "SPen::MotionEvent::GetHistoricalOrientation(int) const":
            self.orientation_reads.append(self.x(1))
        super().hook(u, address, size, data)

    def run(self, case):
        self.history = [tuple(row) for row in case["history"]]
        last = case.get("end", self.history[-1])
        self.event_end = {"x": last[0], "y": last[1], "orientation": last[2],
                          "pressure": case.get("pressure", 0.5), "tilt": case.get("tilt", 0.0),
                          "time": case.get("time", 100), "tool": case.get("tool", 2)}
        self.add_calls = []
        self.orientation_reads = []
        data = self.alloc(256)
        manager = self.alloc(64)
        self.queue = self.alloc(64)
        qvt = self.alloc(32)
        self.put(self.queue, "Q", qvt)
        self.put(qvt + 16, "Q", 0x20001000)
        drawable = self.alloc(512)
        self.call(CTOR, [drawable, data, manager])
        # V1 reads pen size and optional fixed-width state through PenData.
        settings = self.alloc(64)
        self.put(data, "Q", settings)
        self.put(settings, "f", case.get("size", 4.0))
        self.put(data + 9, "B", int(case.get("fixed_enabled", False)))
        self.put(data + 12, "f", case.get("fixed_width", 0.0))
        event, rect = self.alloc(64), self.alloc(32)

        def redraw_and_capture():
            try:
                self.call(REDRAW, [drawable, event, rect], instruction_limit=2_000_000)
            except Exception:
                from unicorn import arm64_const as R
                print("native failure PC", hex(self.u.reg_read(R.UC_ARM64_REG_PC)), file=sys.stderr)
                raise
            rtv1 = self.get(drawable + 16, "Q")[0]
            meshes = []
            for offset in (200, 208, 216):
                vector_address = self.get(rtv1 + offset, "Q")[0]
                vector = self.get(vector_address, "QQQ")
                begin, end, capacity = vector
                if not (begin <= end <= capacity) or (end - begin) % 16:
                    raise RuntimeError(f"native output vector has invalid bounds at {offset}: {vector!r}, vector={vector_address:#x}")
                values = self.get(begin, "f" * ((end - begin) // 4)) if end > begin else ()
                meshes.append({"vertex_f32_bits": [struct.pack("<f", value).hex() for value in values],
                               "vertex_count": (end - begin) // 16})
            return {"orientation_read_indices": self.orientation_reads.copy(),
                    "angle_bits": self.get(drawable + 284, "I")[0].to_bytes(4, "little").hex(),
                    "direction_bits": [self.get(drawable + 276 + 4 * i, "I")[0].to_bytes(4, "little").hex() for i in range(2)],
                    "meshes": meshes, "add_points": self.add_calls.copy()}

        first = redraw_and_capture()
        result = {"fresh": first}
        if "reuse_history" in case:
            # Reuse this same native drawable (no constructor between callbacks).
            self.history = [tuple(row) for row in case["reuse_history"]]
            last = self.history[-1]
            self.event_end = {"x": last[0], "y": last[1], "orientation": last[2],
                              "pressure": case.get("pressure", 0.5), "tilt": case.get("tilt", 0.0),
                              "time": case.get("time", 101), "tool": case.get("tool", 2)}
            self.add_calls, self.orientation_reads = [], []
            result["reused"] = redraw_and_capture()
        return result


def main():
    from pathlib import Path
    library_dir = ROOT / "scratch/apk-analysis-native/arm64-v8a"
    objdump = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump"
    native = NativeCalligraphy(library_dir, objdump)
    cases = [
        {"id": "three_history_samples", "history": [[0.0, 0.0, 0.0], [4.0, 1.0, 0.35], [9.0, 5.0, 0.7]], "reuse_history": [[2.0, 3.0, 0.1], [8.0, 7.0, -0.2], [14.0, 9.0, 0.15]], "size": 6.0},
        {"id": "three_history_zero_orientation", "history": [[0.0, 0.0, 0.0], [4.0, 1.0, 0.0], [9.0, 5.0, 0.0]], "size": 6.0},
    ]
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--fixture", type=Path, default=ROOT / "conformance/calligraphy-saved-native.json")
    args = parser.parse_args()
    output = []
    for case in cases:
        output.append({**case, "native": native.run(case)})
    fixture = {"schema": "sdocx-montblanc-calligraphy-v1-saved-native-v1",
               "evidence": {"library": "libSPenMontblancCalligraphyPen.so", "sha256": LIB_SHA,
                            "entry": REDRAW, "native_scope": "GLV1 RedrawPen history and endPen; one case invokes it twice on the same constructed drawable, then stops before movePen",
                            "inputs": "Synthetic MotionEvent history arrays supplied by this harness, not captured from an application runtime",
                            "host_context": "MotionEvent getters return explicit fixture values; MotionEvent constructor/destructor, RectF bounds operations, queue submit and bounded allocation are host operations",
                            "limitations": ["No APK Java callback construction", "No movePen lifecycle", "No GPU rendering or pixels"]},
               "cases": output}
    encoded = json.dumps(fixture, indent=2) + "\n"
    if args.write_fixture:
        args.fixture.write_text(encoded)
    elif json.loads(args.fixture.read_text()) != fixture:
        raise SystemExit("native output differs from the checked fixture")
    print(f"{len(output)} cases; fixture {args.fixture}")


if __name__ == "__main__":
    main()
