"""Native Pencil3 RTV1 draw handoff oracle with explicit mocked renderer interfaces."""
import argparse
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/pencil3-draw-handoff-native.json"
CALLER = ROOT / "conformance/pencil3_saved_caller_native.py"
ARGB = 0x804080C0
MATRIX = (1.25, 0.125, 0.0, 0.0, -0.25, 0.75, 0.0, 0.0,
          0.0, 0.0, 1.0, 0.0, 13.5, -7.25, 0.0, 1.0)

CASES = {
    "paper-256x128-s0.5": {"canvas": (256, 128), "background_scale": 0.5},
    "paper-256x128-s2": {"canvas": (256, 128), "background_scale": 2.0},
    "paper-640x360-s0.5": {"canvas": (640, 360), "background_scale": 0.5},
    "paper-640x360-s2": {"canvas": (640, 360), "background_scale": 2.0},
    "paper-alt300x90": {"canvas": (640, 360), "background_scale": 2.0,
                         "alternate_background": (300, 90)},
}

PINS = {
    "libSPenBase.so": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libSPenDrawing.so": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "libSPenFountainPen.so": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "libSPenGraphics.so": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "libSPenModel.so": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "libSPenPenCommon.so": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "libSPenPencil3.so": "41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
    "pencil3_saved_caller_native.py": "129001ce587c104e22e1675e759df9a04d942e884a93fb747b195d001b6a776e",
    "pencil3-saved-caller-native.json": "1bf118a229c9e67512ce68d66dd35a2121277121126b1e7cb9b02a387c517121",
    "oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
    "fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "pencil_native.py": "b4d88fb86da22c732e63466e6f1fc7c829aca9d0b2623abbcb3d0fc6f981795c",
}
LIB_NAMES = set(PINS) - {
    "pencil3_saved_caller_native.py", "pencil3-saved-caller-native.json",
    "oblique_canvas_native.py", "fountain_native.py", "elf_plt.py", "pencil_native.py",
}
PIN_PATHS = {name: ROOT / "scratch/apk-analysis-native/arm64-v8a" / name for name in LIB_NAMES}
PIN_PATHS.update({
    "pencil3_saved_caller_native.py": ROOT / "conformance/pencil3_saved_caller_native.py",
    "pencil3-saved-caller-native.json": ROOT / "conformance/pencil3-saved-caller-native.json",
    "oblique_canvas_native.py": ROOT / "conformance/oblique_canvas_native.py",
    "fountain_native.py": ROOT / "conformance/fountain_native.py",
    "elf_plt.py": ROOT / "conformance/elf_plt.py",
    "pencil_native.py": ROOT / "conformance/pencil_native.py",
})

def verify_pins():
    for name, expected in PINS.items():
        actual = hashlib.sha256(PIN_PATHS[name].read_bytes()).hexdigest()
        if actual != expected:
            raise RuntimeError(f"unpinned {name}: {actual}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--write-fixture", action="store_true")
    modes.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--run-case", choices=tuple(CASES), help=argparse.SUPPRESS)
    args = parser.parse_args()
    verify_pins()
    if args.run_case:
        run_case(args.run_case)
        return
    if args.write_fixture:
        fixture = build_fixture()
        FIXTURE.write_text(json.dumps(fixture, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)} ({hashlib.sha256(FIXTURE.read_bytes()).hexdigest()})")
        return
    if not FIXTURE.exists():
        raise SystemExit("Pencil3 draw-handoff fixture missing; use --write-fixture explicitly")
    actual = build_fixture()
    expected = json.loads(FIXTURE.read_text())
    if expected != actual:
        raise SystemExit("Pencil3 draw-handoff fixture mismatch")
    print(f"verified {len(CASES)} Pencil3 native draw-handoff cases; fixture sha256={hashlib.sha256(FIXTURE.read_bytes()).hexdigest()}")


def build_fixture():
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu")
    python_path = [str(ROOT / "scratch/apk-analysis-runtime/python"),
                   str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"),
                   str(ROOT / "conformance")]
    if env.get("PYTHONPATH"):
        python_path.append(env["PYTHONPATH"])
    env["PYTHONPATH"] = os.pathsep.join(python_path)
    results = {}
    for name in CASES:
        command = [sys.executable, str(Path(__file__).resolve()), "--run-case", name]
        run = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, check=True)
        results[name] = json.loads(run.stdout)
    stream_hashes = {row["input_stream_sha256"] for row in results.values()}
    counts = {row["input_particle_count"] for row in results.values()}
    if len(stream_hashes) != 1 or counts != {48}:
        raise RuntimeError("case join lost the common captured selected-V3 stream")
    return {
        "schema": "sdocx-pencil3-draw-handoff-native-v1",
        "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "scope": "Native Drawing-selected V3 stream and native RTV1::Draw binding calls. Renderer interfaces, uploaded texture objects, canvas matrix/size, and GL execution are explicit mocks; no default-constructor, device, or pixel claim.",
        "cases": results,
    }


def run_case(case_name):
    case = CASES[case_name]
    canvas_width, canvas_height = case["canvas"]
    alternate = case.get("alternate_background")
    background_scale = case["background_scale"]
    os.chdir(ROOT)
    sys.path[:0] = [str(ROOT / "conformance"), str(ROOT / "scratch/apk-analysis-runtime/python")]
    os.environ["LD_LIBRARY_PATH"] = str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu")
    import struct
    import io
    import contextlib
    source_bytes = CALLER.read_bytes()
    source_hash = hashlib.sha256(source_bytes).hexdigest()
    if source_hash != PINS["pencil3_saved_caller_native.py"]:
        raise RuntimeError("caller source hash changed during run")
    source = source_bytes.decode("utf-8")
    needle = "p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);"
    if source.count(needle) != 1:
        raise RuntimeError(f"expected one caller injection site; found {source.count(needle)}")
    injection = f"p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,{ARGB}]); " + needle
    patched = source.replace(needle, injection)
    if patched.count(injection) != 1 or patched.count(needle) != 1:
        raise RuntimeError("in-memory SetColor patch did not apply exactly once")
    caller_argv = [str(CALLER), "--print-capture", "--advanced-setting", "empty",
                   "--width", str(canvas_width), "--height", str(canvas_height)]
    saved_argv = sys.argv
    sys.argv = caller_argv
    output = io.StringIO()
    try:
        with contextlib.redirect_stdout(output):
            state = {"__name__": "__main__", "__file__": str(CALLER)}
            exec(compile(patched, str(CALLER), "exec"), state)
    finally:
        sys.argv = saved_argv
    caller = json.loads(output.getvalue())
    p = state["p"]
    rt = state["selected_rt"]
    R = state["R"]
    from unicorn import UC_HOOK_CODE

    def execute_case():
        p = state["p"]
        rt = state["selected_rt"]
        stop = state["oc"].STOP
        R = state["R"]
        from unicorn import UC_HOOK_CODE

        # The selected native Drawing route supplied the packed V3 stream. Draw's RT+88
        # descriptor is cleared on caller return, so materialize those exact bytes at
        # the renderer handoff boundary, as in the previously reviewed paint probes.
        stream = state["saved_stream"]
        assert len(stream) == caller["streams"]["saved_redraw"]["float_count"] * 4
        assert hashlib.sha256(stream).hexdigest() == caller["streams"]["saved_redraw"]["sha256"]
        assert caller["route"]["selected_drawable_class"] == "SPen::Pencil3StrokeDrawableGLV3"
        vector = p.alloc(24)
        vector_data = p.alloc(len(stream))
        p.u.mem_write(vector_data, stream)
        p.put(vector, "3Q", vector_data, vector_data + len(stream), vector_data + len(stream))
        p.put(rt + 88, "Q", vector)
        p.put(rt + 72, "I", caller["streams"]["saved_redraw"]["callback_count"])

        # The in-memory caller variant supplied ARGB through ObjectStroke::SetColor;
        # native Drawing forwarded it through PenDrawableRT::SetPenData into the RT.
        color = tuple(p.get(rt + 44, "4f"))
        p.put(rt + 264, "f", background_scale)

        events = []
        recent_pcs = []
        def trace_pcs(_uc, address, _size, _data):
            recent_pcs.append(address)
            if len(recent_pcs) > 80:
                del recent_pcs[:-80]
        p.u.hook_add(UC_HOOK_CODE, trace_pcs)
        next_address = 0x200F3000
        def hook_for(label):
            nonlocal next_address
            addr = next_address
            next_address += 0x10
            def hook(_uc, address, _size, _data):
                if address != addr:
                    return
                x0, x1, x2 = p.x(0), p.x(1), p.x(2)
                row = {"event": label, "x0": hex(x0), "x1": hex(x1), "x2": hex(x2)}
                if label.startswith("uniform:"):
                    name = label.split(":", 1)[1]
                    if name == "uProjectionMatrix":
                        row["matrix_f32"] = list(p.get(x1, "16f"))
                        row["matrix_f32_bits"] = [p.u.mem_read(x1 + i * 4, 4).hex() for i in range(16)]
                    elif name == "inputColor":
                        row["rgba_f32"] = list(p.get(x1, "4f"))
                        row["rgba_f32_bits"] = [p.u.mem_read(x1 + i * 4, 4).hex() for i in range(4)]
                    elif name == "uPaperScale":
                        row["xy_f32"] = list(p.get(x1, "2f"))
                        row["xy_f32_bits"] = [p.u.mem_read(x1 + i * 4, 4).hex() for i in range(2)]
                    elif name in ("pointPattern", "paperTexture"):
                        row["texture_unit_i32"] = p.get(x1, "i")[0]
                    events.append(row)
                elif label == "canvas:GetMatrix4":
                    out = p.u.reg_read(R.UC_ARM64_REG_X8)
                    p.put(out, "16f", 1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1)
                    row["out"] = hex(out)
                    row["matrix_f32"] = list(p.get(out, "16f"))
                    events.append(row)
                elif label.startswith("texture:"):
                    events.append(row)
                elif label == "subbitmap:GetWidth":
                    p.x(0, canvas_width)
                    row["return"] = canvas_width
                    events.append(row)
                elif label == "subbitmap:GetHeight":
                    p.x(0, canvas_height)
                    row["return"] = canvas_height
                    events.append(row)
                elif label == "blend:Activate":
                    events.append(row)
                elif label == "draw:DrawInstanced":
                    row["args"] = [p.x(i) for i in range(4)]
                    events.append(row)
                elif label.startswith("subbitmap:") or label == "depth:Disable":
                    events.append(row)
                p.u.reg_write(R.UC_ARM64_REG_PC, p.u.reg_read(R.UC_ARM64_REG_LR))
            p.u.hook_add(UC_HOOK_CODE, hook)
            return addr

        def vtable_object(size=256):
            obj, vt = p.alloc(32), p.alloc(size)
            p.put(obj, "Q", vt)
            return obj, vt

        def install_slot(vt, byte_offset, label, result_pointer=None):
            addr = hook_for(label)
            if result_pointer is not None:
                # The callback needs a per-slot pointer result.
                # Set the pointer through an attribute read from the object at call time.
                # Hook uses this slot's assigned result via a side table below.
                result_by_address[addr] = result_pointer
            p.put(vt + byte_offset, "Q", addr)
            return addr

        result_by_address = {}

        # Install caller-return subbitmap with the exact arguments Draw consumes.
        sub, subvt = vtable_object()
        install_slot(subvt, 16, "canvas:GetMatrix4")
        install_slot(subvt, 40, "subbitmap:Begin")
        install_slot(subvt, 48, "subbitmap:Restore")
        install_slot(subvt, 96, "subbitmap:GetWidth")
        install_slot(subvt, 104, "subbitmap:GetHeight")

        # Capture texture binding calls; fake texture objects stand for already-uploaded
        # source assets. This probe tests Draw's selection/units, not texture upload.
        textures = {}
        for label in ("brush", "paper"):
            bmp, vt = vtable_object()
            bind = hook_for("texture:" + label + ":Bind")
            p.put(vt + 16, "Q", bind)
            textures[label] = bmp
            owner, ownervt = vtable_object()
            get_bitmap = hook_for("bitmap:GetTextureBitmap")
            result_by_address[get_bitmap] = bmp
            p.put(ownervt + 144, "Q", get_bitmap)
            if label == "brush":
                p.put(rt + 232, "Q", owner)
            else:
                p.put(rt + 240, "Q", owner)

        if alternate:
            alternate_bitmap, alternate_vt = vtable_object()
            p.put(alternate_vt + 16, "Q", hook_for("texture:alternate-background:Bind"))
            alternate_owner, alternate_owner_vt = vtable_object()
            get_alternate = hook_for("bitmap:alternate-background:GetTextureBitmap")
            result_by_address[get_alternate] = alternate_bitmap
            p.put(alternate_owner_vt + 144, "Q", get_alternate)
            p.put(rt + 248, "Q", alternate_owner)
            p.put(rt + 256, "I", alternate[0])
            p.put(rt + 260, "I", alternate[1])

        # Shader wrapper contains the program at +40 and four real native helper calls
        # will dispatch through these parameter-binding vtables.
        shader = p.alloc(64)
        program, programvt = vtable_object()
        use = hook_for("shader:Use")
        p.put(programvt + 16, "Q", use)
        p.put(shader + 40, "Q", program)
        for offset, name in ((0, "uProjectionMatrix"), (8, "pointPattern"), (16, "paperTexture"), (24, "inputColor"), (32, "uPaperScale")):
            binding, vt = vtable_object()
            method = hook_for("uniform:" + name)
            p.put(vt, "Q", method)
            p.put(shader + offset, "Q", binding)
        p.put(rt + 216, "Q", shader)

        # The actual RTV::Draw accesses its blend, graphics and texture vtables.
        blend, blendvt = vtable_object()
        p.put(blendvt + 24, "Q", hook_for("blend:Activate"))
        p.put(rt + 224, "Q", blend)
        graphics, graphicsvt = vtable_object()
        p.put(graphicsvt + 56, "Q", hook_for("draw:DrawInstanced"))
        p.put(rt + 208, "Q", graphics)

        # Native Draw asks the drawable base to disable depth/stencil. It is a PLT
        # import, so retain only this explicit no-op host boundary.
        p.callbacks["SPen::PenDrawableRT::DisableDepthStencil()"] = lambda: events.append({"event":"depth:Disable"})
        p.callbacks["posix_memalign"] = lambda: (p.put(p.x(0), "Q", p.alloc(p.x(2))), p.x(0, 0))
        binding_type = p.alloc(4)
        p.put(binding_type, "I", 2)
        p.callbacks["SPen::IParameterBinding::GetType() const"] = lambda: p.x(0, binding_type)

        # Hook results for native virtual calls that return pointers.
        original_hook = None
        def result_hook(_uc, address, _size, _data):
            if address in result_by_address:
                p.x(0, result_by_address[address])
                p.u.reg_write(R.UC_ARM64_REG_PC, p.u.reg_read(R.UC_ARM64_REG_LR))
        p.u.hook_add(UC_HOOK_CODE, result_hook)

        # Nonidentity caller matrix exercises native matrix composition and UV scale.
        matrix = p.alloc(64)
        p.put(matrix, "16f", 1.25,0.125,0,0, -0.25,0.75,0,0, 0,0,1,0, 13.5,-7.25,0,1)
        try:
            p.call("SPen::Pencil3StrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)", [rt, sub, matrix], limit=3_000_000)
        except Exception as exc:
            print(json.dumps({"stage":"Draw","error":repr(exc),"pc":hex(p.u.reg_read(R.UC_ARM64_REG_PC)),"lr":hex(p.u.reg_read(R.UC_ARM64_REG_LR)),"sp":hex(p.u.reg_read(R.UC_ARM64_REG_SP)),"regs":[hex(p.x(i)) for i in range(10)],"recent_pcs":[hex(x) for x in recent_pcs],"events":events},indent=2))
            raise

        report = {
          "scope":"Actual pinned Pencil3StrokeDrawableRTV1::Draw executes in Unicorn over the actual Drawing-selected V3 AddPoint stream. Renderer interfaces, uploaded resource objects, the host canvas matrix, and GL execution are explicit mocks; this captures native handoff values, not device pixels.",
          "pinned_inputs_sha256": PINS,
          "case_id": case_name,
          "caller_source_sha256": source_hash,
          "in_memory_patch": {"site_count": 1, "replacement_sha256": hashlib.sha256(injection.encode()).hexdigest(), "argb_u32": hex(ARGB)},
          "caller_fixture_sha256": hashlib.sha256((ROOT/"conformance/pencil3-saved-caller-native.json").read_bytes()).hexdigest(),
          "caller_host_boundaries": caller["host_boundaries"],
          "selected_route": caller["route"],
          "input_stream_sha256": caller["streams"]["saved_redraw"]["sha256"],
          "input_particle_count": caller["streams"]["saved_redraw"]["callback_count"],
          "canvas_profile": {"width":canvas_width,"height":canvas_height,"matrix_f32":list(p.get(matrix,"16f")),"matrix_f32_bits":[p.u.mem_read(matrix+i*4,4).hex() for i in range(16)], "host_matrix_returned_by_mock": list(p.get(matrix,"16f"))},
          "source_argb_u32": hex(ARGB),
          "source_color_forwarding": state["paint_color_calls"],
          "rt_color_after_native_SetPenData_f32": list(p.get(rt+44,"4f")),
          "rt_background_scale_supplied_f32": p.get(rt+264,"f")[0],
          "alternate_background_dimensions": list(alternate) if alternate else None,
          "init_static_contract": {
              "source": "libSPenPencil3.so SPen::Pencil3StrokeDrawableRTV1::Init at ELF VA 0x10c4b8-0x10c4f8",
              "blend_enabled": True,
              "color_blend_function_enum": 0,
              "color_source_factor_enum": 6,
              "color_destination_factor_enum": 7,
              "alpha_blend_function_enum": 0,
              "alpha_source_factor_enum": 1,
              "alpha_destination_factor_enum": 7,
              "texture_dimensions": [120, 120],
              "texture_atlas_rows": 16,
          },
          "shader_sources": {
              "vertex_rva": "0xf9730",
              "fragment_rva": "0xf9948",
              "uniforms": ["uProjectionMatrix", "pointPattern", "paperTexture", "inputColor", "uPaperScale"],
              "fragment_alpha": "inputColor.a * brushMask * paper * vAlpha * 1.5",
              "fragment_rgb": "inputColor.rgb",
              "paper_uv": "vCanvasUV * uPaperScale",
              "brush_uv": "vPointUV",
          },
          "events": events,
        }
        if caller["route"]["selected_drawable_class"] != "SPen::Pencil3StrokeDrawableGLV3":
            raise RuntimeError("caller did not select V3")
        if caller["streams"]["saved_redraw"]["callback_count"] != 48:
            raise RuntimeError("unexpected saved V3 particle count")
        set_color = [row for row in state["paint_color_calls"] if row["method"] == "SPen::Pen::SetColor(int)"]
        set_data = [row for row in state["paint_color_calls"] if row["method"] == "SPen::PenDrawableRT::SetPenData(float, int)"]
        if len(set_color) != 3 or len(set_data) != 2:
            raise RuntimeError("native color forwarding call cardinality changed")
        if not any(row["phase"] == "draw_object_stroke" and row["argb_u32"] == hex(ARGB) for row in set_color):
            raise RuntimeError("injected ObjectStroke color did not reach native Pen::SetColor")
        if any(row["argb_u32"] != hex(ARGB) for row in set_data):
            raise RuntimeError("ARGB did not survive native PenDrawableRT::SetPenData")
        if len([row for row in events if row["event"] == "draw:DrawInstanced"]) != 1:
            raise RuntimeError("expected exactly one native DrawInstanced handoff")
        expected_events = ["blend:Activate", "depth:Disable", "subbitmap:Begin", "canvas:GetMatrix4",
                           "uniform:uProjectionMatrix", "uniform:inputColor", "texture:brush:Bind",
                           "uniform:pointPattern", "texture:paper:Bind" if not alternate else "texture:alternate-background:Bind",
                           "subbitmap:GetWidth", "subbitmap:GetHeight", "uniform:paperTexture",
                           "uniform:uPaperScale", "draw:DrawInstanced", "subbitmap:Restore"]
        if [row["event"] for row in events] != expected_events:
            raise RuntimeError(f"native Draw callback order/cardinality changed: {[row['event'] for row in events]}")
        return report
    report = execute_case()
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
