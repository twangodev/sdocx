#!/usr/bin/env python3
"""Pinned native source-to-stream conformance for synthetic saved InkPen2 strokes."""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import shutil
import struct
import subprocess
import sys
import tempfile
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
BASE_CALLER = ROOT / "conformance/inkpen2_saved_caller_native.py"
EXPECTED_CALLER_SHA256 = "a1d0f06b88f5de50274f81228f04020231a7d23089581fbee99252648a3676aa"
EXPECTED_APK_SHA256 = "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
EXPECTED_MODEL_SHA256 = "888f1e795bb4b07b39fc583f6355ac3f6a9fbe7fa05c66d96911561d8f2335a4"
EXPECTED_PATH_SHA256 = "db4e6a9a97fd0b7407f90334a76edbb3a05a97a9faeb2803a6bb03a4032aa2cf"
PROFILE = (1920, 1080)
EXPECTED_CASES = {"two_point_short", "duplicate_mid", "pressure_extremes", "size_floor", "size_over800", "linear", "single_point"}
HELPER_SHA256 = {
    "conformance/elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "conformance/fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "conformance/oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
}
BASE_FIXTURE = ROOT / "conformance/inkpen2-saved-caller-native.json"
FIXTURE = ROOT / "conformance/inkpen2-source-caller-native.json"
MODEL_RS = ROOT / "crates/sdocx/src/ink/inkpen2.rs"
MODEL_PATH = ROOT / "crates/sdocx/src/ink/path.rs"
PINS_FILE = ROOT / "conformance/inkpen2-source-provenance-pins.json"
RUSTC_PATH: pathlib.Path | None = None

# Each case is a synthetic ObjectStroke setter profile. The native getter
# snapshots, not these requests, are the source of model inputs below.
CASES = {
    "two_point_short": {"points": [[0., 0.], [.75, 0.]], "size": 4., "curve_enabled": True, "tool_type": 2, "stroke_type": 0, "pressure": [.2, .8], "timestamps": [100, 105], "tilt": [0., 0.], "orientation": [0., 0.]},
    "duplicate_mid": {"points": [[0., 0.], [8., 0.], [8., 0.], [16., 3.], [24., 3.]], "size": 4., "curve_enabled": True, "tool_type": 2, "stroke_type": 0, "pressure": [.2, .4, .9, .6, .8], "timestamps": [100, 110, 120, 130, 140], "tilt": [0.] * 5, "orientation": [0.] * 5},
    "pressure_extremes": {"points": [[0., 0.], [8., 0.], [16., 3.], [24., 3.]], "size": 4., "curve_enabled": True, "tool_type": 2, "stroke_type": 0, "pressure": [0., 1., 0., 1.], "timestamps": [0, 10, 20, 30], "tilt": [0.] * 4, "orientation": [0.] * 4},
    "size_floor": {"points": [[0., 0.], [8., 0.], [16., 3.], [24., 3.]], "size": .25, "curve_enabled": True, "tool_type": 2, "stroke_type": 0, "pressure": [.2, .4, .6, .8], "timestamps": [0, 10, 20, 30], "tilt": [0.] * 4, "orientation": [0.] * 4},
    "size_over800": {"points": [[0., 0.], [8., 0.], [16., 3.], [24., 3.]], "size": 801., "curve_enabled": True, "tool_type": 2, "stroke_type": 0, "pressure": [.2, .4, .6, .8], "timestamps": [0, 10, 20, 30], "tilt": [0.] * 4, "orientation": [0.] * 4},
    "linear": {"points": [[0., 0.], [8., 0.], [16., 3.], [24., 3.]], "size": 4., "curve_enabled": False, "tool_type": 2, "stroke_type": 0, "pressure": [.2, .4, .6, .8], "timestamps": [0, 10, 20, 30], "tilt": [0.] * 4, "orientation": [0.] * 4},
    "single_point": {"points": [[3., 4.]], "size": 4., "curve_enabled": True, "tool_type": 2, "stroke_type": 0, "pressure": [.7], "timestamps": [100], "tilt": [0.], "orientation": [0.]},
}


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def runtime_closure(pins: dict) -> dict:
    global RUSTC_PATH
    runtime_python = ROOT / "scratch/apk-analysis-runtime/python"
    sys.path.insert(0, str(runtime_python))
    import unicorn
    import unicorn.unicorn_py3.unicorn as unicorn_backend

    python_path = pathlib.Path(sys.executable).resolve()
    package_dir = pathlib.Path(unicorn.__file__).resolve().parent
    unicorn_library = pathlib.Path(unicorn_backend.uclib._name).resolve()
    runtime_files = {
        "python_executable": python_path,
        "unicorn_package_init": package_dir / "__init__.py",
        "unicorn_python_module": package_dir / "unicorn.py",
        "unicorn_backend_module": package_dir / "unicorn_py3/unicorn.py",
        "unicorn_native_library": unicorn_library,
    }
    runtime_hashes = {key: sha256(path) for key, path in runtime_files.items()}
    rustc_path = pathlib.Path(subprocess.check_output(["rustup", "which", "rustc"], text=True).strip()).resolve()
    RUSTC_PATH = rustc_path
    tools = {
        "nm": pathlib.Path(shutil.which("nm") or "").resolve(),
        "c++filt": pathlib.Path(shutil.which("c++filt") or "").resolve(),
        "rustc": rustc_path,
        "objdump": ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump",
    }
    tool_records = {}
    tool_env = os.environ.copy()
    tool_env["LD_LIBRARY_PATH"] = str((ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu").resolve())
    for name, path in tools.items():
        if not path.is_file():
            raise RuntimeError(f"required runtime executable is missing: {name} at {path}")
        version = subprocess.check_output([str(path), "--version"], text=True, env=tool_env).splitlines()[0]
        tool_records[name] = {"path": str(path), "sha256": sha256(path), "version": version}

    expected = pins["runtime"]
    actual_identity = {
        "python_version": platform.python_version(),
        "unicorn_version": unicorn.__version__,
        "runtime_files_sha256": runtime_hashes,
        "tool_sha256": {name: row["sha256"] for name, row in tool_records.items()},
        "tool_versions": {name: row["version"] for name, row in tool_records.items()},
    }
    if actual_identity != expected:
        raise RuntimeError(f"runtime closure differs from preflight pins: {json.dumps(actual_identity, sort_keys=True)}")
    return {
        "python": {"path": str(python_path), "version": platform.python_version(), "sha256": runtime_hashes["python_executable"]},
        "unicorn": {
            "version": unicorn.__version__,
            "package_path": str(package_dir.relative_to(ROOT)),
            "package_init_sha256": runtime_hashes["unicorn_package_init"],
            "python_module_path": str(runtime_files["unicorn_python_module"].relative_to(ROOT)),
            "python_module_sha256": runtime_hashes["unicorn_python_module"],
            "backend_module_path": str(runtime_files["unicorn_backend_module"].relative_to(ROOT)),
            "backend_module_sha256": runtime_hashes["unicorn_backend_module"],
            "native_library_path": str(unicorn_library.relative_to(ROOT)),
            "native_library_sha256": runtime_hashes["unicorn_native_library"],
        },
        "tools": tool_records,
    }


def pin_inputs() -> dict:
    pins = json.loads(PINS_FILE.read_text())
    runner_sha256 = sha256(pathlib.Path(__file__).resolve())
    if runner_sha256 != pins["runner_sha256"]:
        raise RuntimeError("runner source SHA differs from provenance-pins.json before native execution")
    assert sha256(BASE_CALLER) == EXPECTED_CALLER_SHA256, "saved caller source changed"
    helpers = {name: sha256(ROOT / name) for name in HELPER_SHA256}
    assert helpers == HELPER_SHA256, f"native helper source pin mismatch: {helpers}"
    base = json.loads(BASE_FIXTURE.read_text())
    base_case = next(iter(base["cases"].values()))
    libraries = base_case["libraries"]
    apk_libdir = ROOT / "scratch/apk-analysis-native/arm64-v8a"
    libfiles = {
        "Base": "libSPenBase.so", "Composer": "libSPenComposer.so",
        "Drawing": "libSPenDrawing.so", "FountainPen": "libSPenFountainPen.so",
        "Graphics": "libSPenGraphics.so", "InkPen2": "libSPenInkPen2.so",
        "Model": "libSPenModel.so", "PenCommon": "libSPenPenCommon.so",
        "libSPenComposer.so": "libSPenComposer.so", "libc++_shared.so": "libc++_shared.so",
    }
    actual = {name: sha256(apk_libdir / libfiles[name]) for name in libfiles}
    assert actual == libraries, "native DSO pin mismatch before probe execution"
    apk = ROOT / "scratch/apk-input/samsung-notes-4.4.45.37.apk"
    apk_hash = sha256(apk)
    assert apk_hash == EXPECTED_APK_SHA256, "APK pin mismatch before probe execution"
    assert sha256(MODEL_RS) == EXPECTED_MODEL_SHA256, "numeric core source changed"
    assert sha256(MODEL_PATH) == EXPECTED_PATH_SHA256, "SmPath source changed"
    runtime = runtime_closure(pins)
    return {"apk_sha256": apk_hash, "native_dsos_sha256": actual, "helper_sources_sha256": helpers,
            "saved_caller_source_sha256": EXPECTED_CALLER_SHA256, "numeric_model_source_sha256": EXPECTED_MODEL_SHA256,
            "path_source_sha256": EXPECTED_PATH_SHA256, "runner_sha256": runner_sha256,
            "runtime_closure": runtime, "provenance_pins_sha256": sha256(PINS_FILE)}


def generated_probe(source: str, root: pathlib.Path) -> str:
    source = source.replace("ROOT=pathlib.Path(__file__).resolve().parents[1]", f"ROOT=pathlib.Path({str(root)!r})")
    source = source.replace("parser.add_argument('--case',choices=", "parser.add_argument('--input-json',type=pathlib.Path)\nparser.add_argument('--case',choices=")
    source = source.replace("input_case=INPUT_CASES[args.case]", "input_case=INPUT_CASES[args.case]\nif args.input_json: input_case.update(json.loads(args.input_json.read_text()))")
    source = source.replace("p=oc.CanvasProbe()", "p=oc.CanvasProbe()\nimport math\np.callbacks['fmodf']=lambda:p.f(0,math.nan if p.f(1)==0.0 else math.fmod(p.f(0),p.f(1)))")
    source = source.replace("'stroke_type':args.stroke_type", "'stroke_type':input_case['stroke_type']")
    source = source.replace("source_color_argb=None", "source_color_argb=None; p.call('SPen::ObjectStroke::GetColor() const',[obj]); source_color_default_getter=p.x(0)")
    source = source.replace(" if source_color_argb is not None:\n  result['input']['color_argb_u32']", " result['native_calls']['source_ObjectStroke_GetColor_default_u32']=source_color_default_getter\n if source_color_argb is not None:\n  result['input']['color_argb_u32']")
    pattern = r" if not factory_created_pens:\n  print\(json\.dumps\(\{.*?raise SystemExit\(0\)"
    new = """ if not factory_created_pens:
  rejected={'schema':'inkpen2-saved-caller-rejection-v1','case_id':args.case,'input':{'kind':'synthetic native ObjectStroke setter input; no serialized SDocx parser input','pen_name':'com.samsung.android.sdk.pen.pen.preload.InkPen2','advanced_getter_pointer_nonnull':bool(advanced_getter),'advanced_getter_object_hex':advanced_getter_bytes,'advanced_getter_utf8':advanced_utf8,'stroke_type':input_case['stroke_type'],'tool_type':input_case['tool_type'],'curve_enabled':input_case['curve_enabled'],'pen_size_f32_bits':source_size_bits,'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in points],'pressure_f32_bits':[struct.pack('<f',v).hex() for v in input_case['pressure']],'timestamps_i32':input_case['timestamps'],'tilt_f32_bits':[struct.pack('<f',v).hex() for v in input_case['tilt']],'orientation_f32_bits':[struct.pack('<f',v).hex() for v in input_case['orientation']],'advanced_setting_state':args.advanced_setting,'advanced_setting':'9;'},'profile':{'width':args.width,'height':args.height,'canvas_model_matrix_f32_bits':[struct.pack('<f',v).hex() for v in (1,0,0,0,1,0,0,0,1)]},'native_calls':{'ObjectDrawing::drawObjectStroke_return':draw_result,'factory_count':0,'selected_drawable':None,'selected_drawable_version':None,'source_ObjectStroke_arrays':source_object_arrays,'source_ObjectStroke_GetPenSize_f32_bits':source_size_bits,'ObjectStroke_getter_snapshots':object_stroke_snapshots,'pen_screen_resolution':screen_resolution_calls,'screen_profile_requested_by_context':{'width':args.width,'height':args.height},'screen_setup_reached':bool(screen_resolution_calls),'loader':loader_calls,'source_ObjectStroke_GetColor_default_u32':source_color_default_getter,'RedrawPen_entries':[r for r in native_draw_entries if 'RedrawPen(' in ' '.join(r['signatures'])]},'native_method_entries':native_draw_entries,'callback_buffer_events':inkpen2_buffer_events,'libraries':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in paths.items()},'scope':'Actual Drawing route rejection before InkPen2 factory selection; explicit synthetic profile and ObjectStroke setters. No serialized SDocx parser or device-default claim.'}
  rejected['libraries']['libc++_shared.so']=hashlib.sha256(CPP.read_bytes()).hexdigest(); rejected['libraries']['libSPenComposer.so']=hashlib.sha256((LIBS/'libSPenComposer.so').read_bytes()).hexdigest()
  print(json.dumps(rejected,indent=2),_always=True); raise SystemExit(0)"""
    source, count = re.subn(pattern, lambda _: new, source, count=1, flags=re.S)
    if count != 1:
        raise RuntimeError("expected factory rejection boundary not found in saved caller")
    return source


def array_values(record: dict, fmt: str) -> list:
    arr = record["data_hex"]
    return [x[0] if len(x) == 1 else x for x in struct.iter_unpack(fmt, bytes.fromhex(arr))]


def f32_bits(value: float) -> str:
    return struct.pack("<f", value).hex()


def assert_getter_capture(case: dict) -> None:
    n = case["native_calls"]
    requested = case["input"]
    arrays = n["source_ObjectStroke_arrays"]
    getter = n["ObjectStroke_getter_snapshots"][0]
    pts = array_values(arrays["points"], "<ff")
    assert [[f32_bits(x), f32_bits(y)] for x, y in pts] == requested["points_f32_bits"]
    assert [f32_bits(v) for v in array_values(arrays["pressures"], "<f")] == requested["pressure_f32_bits"]
    assert array_values(arrays["timestamps"], "<i") == requested["timestamps_i32"]
    assert [f32_bits(v) for v in array_values(arrays["tilts"], "<f")] == requested["tilt_f32_bits"]
    assert [f32_bits(v) for v in array_values(arrays["orientations"], "<f")] == requested["orientation_f32_bits"]
    assert getter["point_count"] == len(pts) and getter["stroke_type"] == requested["stroke_type"]
    assert getter["tool_type"] == requested["tool_type"] and bool(getter["curve_enabled"]) == requested["curve_enabled"]
    assert getter["size_bits"] == requested["pen_size_f32_bits"] == n["source_ObjectStroke_GetPenSize_f32_bits"]
    assert requested["advanced_getter_utf8"] == "9;" and case["profile"]["width"] == PROFILE[0] and case["profile"]["height"] == PROFILE[1]


def native_model_input(case: dict) -> dict | None:
    n = case["native_calls"]
    if n.get("factory_count") == 0:
        return None
    arrays = n["source_ObjectStroke_arrays"]
    points = array_values(arrays["points"], "<ff")
    times = array_values(arrays["timestamps"], "<i")
    pressures = array_values(arrays["pressures"], "<f")
    getters = n["ObjectStroke_getter_snapshots"]
    getter = next(row for row in getters if row["label"] == "before_drawObjectStroke")
    entry = next(row for row in n["selected_drawable_redraw_entry_state"] if row["phase"] == "draw_object_stroke")
    profile = case["profile"]
    requested = case["input"]
    def bits_floats(values):
        return [struct.pack("<f", v).hex() for v in values]
    assert [[struct.pack("<f", v).hex() for v in xy] for xy in points] == requested["points_f32_bits"]
    assert bits_floats(pressures) == requested["pressure_f32_bits"]
    assert times == requested["timestamps_i32"]
    assert getter["point_count"] == len(points) and getter["stroke_type"] == requested["stroke_type"]
    assert getter["tool_type"] == requested["tool_type"] and bool(getter["curve_enabled"]) == requested["curve_enabled"]
    assert n["source_ObjectStroke_GetPenSize_f32_bits"] == requested["pen_size_f32_bits"]
    assert len(n["pen_screen_resolution"]) == 1
    assert n["pen_screen_resolution"][0] == {"width": profile["width"], "height": profile["height"]}
    size = struct.unpack("<f", bytes.fromhex(n["drawable_getSize_f32_bits"]))[0]
    inverse = [struct.unpack("<f", bytes.fromhex(b))[0] for b in n["drawable_getInverseScale_f32_bits"]]
    spacing = struct.unpack("<f", bytes.fromhex(entry["spacing_plus_88_f32_bits"]))[0]
    tolerance_entry = next(row for row in n["drawline_threshold_entries"] if row["phase"] == "draw_object_stroke")
    tolerance = struct.unpack("<f", bytes.fromhex(tolerance_entry["plus44_tolerance_f32_bits"]))[0]
    threshold = struct.unpack("<f", bytes.fromhex(entry["tool_threshold_plus_96_f32_bits"]))[0]
    color = n.get("source_ObjectStroke_GetColor_u32", n["source_ObjectStroke_GetColor_default_u32"])
    return {"points": points, "times": times, "pressures": pressures,
            "tools": [int(getter["tool_type"])] * len(points), "size": size,
            "curve_enabled": bool(getter["curve_enabled"]), "stroke_type": int(getter["stroke_type"]),
            "rgb": [(color >> 16) & 255, (color >> 8) & 255, color & 255],
            "runtime": {"inverse": inverse, "spacing": spacing, "tolerance": tolerance,
                        "threshold": threshold, "resolution_min": min(n["pen_screen_resolution"][0]["width"], n["pen_screen_resolution"][0]["height"]),
                        "initial_alternation": bool(entry["alternation_plus_100_u8"]), "initial_time": times[0]},
            "captured_source_size_bits": n["source_ObjectStroke_GetPenSize_f32_bits"],
            "captured_selected_size_bits": n["drawable_getSize_f32_bits"],
            "captured_profile": profile}


def rust_model_replay(cases: dict) -> dict:
    model_rows = {}
    src = []
    for name, result in cases.items():
        model = native_model_input(result)
        if model is None:
            model_rows[name] = {"native_factory_rejected": True, "model_not_invoked": True}
            continue
        if model["stroke_type"] != 0:
            raise RuntimeError(f"unsupported source stroke type in intended case {name}")
        points = ",".join(f"[{x!r}f32,{y!r}f32]" for x, y in model["points"])
        times = ",".join(map(str, model["times"]))
        pressures = ",".join(f"{v!r}f32" for v in model["pressures"])
        tools = ",".join(map(str, model["tools"]))
        inv = model["runtime"]["inverse"]
        rgb = model["rgb"]
        rt = model["runtime"]
        stmt = """{ let points=vec![@POINTS@].into_iter().map(|v|P{x:v[0],y:v[1]}).collect::<Vec<_>>();
let times=[@TIMES@]; let pressures=[@PRESSURES@]; let tools=[@TOOLS@];
let out=inkpen2::redraw(inkpen2::Input{points:&points,times:&times,tools:&tools,pressures:&pressures,rgb:[@R0@,@R1@,@R2@],size:@SIZE@,curve_enabled:@CURVE@,is_shape:false,runtime:inkpen2::Runtime{inverse_scale:[@IX@,@IY@],spacing:@SPACING@,tolerance:@TOLERANCE@,tool_threshold:@THRESHOLD@,resolution_min:@RESOLUTION@,initial_alternation:@ALT@,initial_time:@INITIAL_TIME@}});
match out{Some(o)=>{print!("@NAME@ ");for row in o.vertices{for f in row{print!("{:08x}",f.to_bits())}}println!();},None=>println!("@NAME@ REJECT")} }
"""
        values = {"POINTS": points, "TIMES": times, "PRESSURES": pressures, "TOOLS": tools,
                  "R0": str(rgb[0]), "R1": str(rgb[1]), "R2": str(rgb[2]), "SIZE": f"{model['size']!r}f32",
                  "CURVE": str(model["curve_enabled"]).lower(), "IX": f"{inv[0]!r}f32", "IY": f"{inv[1]!r}f32",
                  "SPACING": f"{rt['spacing']!r}f32", "TOLERANCE": f"{rt['tolerance']!r}f32",
                  "THRESHOLD": f"{rt['threshold']!r}f32", "RESOLUTION": str(rt["resolution_min"]),
                  "ALT": str(rt["initial_alternation"]).lower(), "INITIAL_TIME": str(rt["initial_time"]), "NAME": name}
        for key, value in values.items():
            stmt = stmt.replace("@" + key + "@", value)
        src.append(stmt)
        model_rows[name] = {"source_size_bits": model["captured_source_size_bits"], "selected_size_bits": model["captured_selected_size_bits"],
                            "screen_profile": model["captured_profile"], "runtime": rt}
    wrapper = f'''#[derive(Clone,Copy)] pub struct Point{{x:f64,y:f64}}
#[path={json.dumps(str(MODEL_PATH))}] mod path;
#[path={json.dumps(str(MODEL_RS))}] mod inkpen2;
use path::P;
fn main(){{ {''.join(src)} }}
'''
    # Rust attributes require quoted paths; Python repr is valid for these Unix paths.
    with tempfile.TemporaryDirectory(prefix="inkpen2-model-") as td:
        td = pathlib.Path(td)
        rs = td / "model.rs"
        exe = td / "model"
        rs.write_text(wrapper)
        if RUSTC_PATH is None:
            raise RuntimeError("runtime preflight did not resolve the pinned Rust compiler")
        compile_result = subprocess.run([str(RUSTC_PATH), "--edition=2024", str(rs), "-o", str(exe)], cwd=ROOT, text=True, capture_output=True)
        if compile_result.returncode:
            raise RuntimeError(f"numeric core compile failed:\n{compile_result.stderr}")
        run = subprocess.run([str(exe)], cwd=ROOT, text=True, capture_output=True)
        if run.returncode:
            raise RuntimeError(f"numeric core execution failed:\n{run.stderr}")
    outputs = {}
    for line in run.stdout.splitlines():
        name, words = line.split(" ", 1)
        outputs[name] = b"".join(struct.pack("<I", int(words[i:i+8], 16)) for i in range(0, len(words), 8))
    for name, expected in cases.items():
        model = native_model_input(expected)
        if model is None:
            model_rows[name]["native_rejection_evidence"] = {"draw_result": expected["native_calls"]["ObjectDrawing::drawObjectStroke_return"], "factory_count": 0}
            continue
        events = [e for e in expected["callback_buffer_events"] if e["phase"] == "draw_object_stroke"]
        actual = bytes.fromhex(events[-1]["after_bytes"])
        predicted = outputs[name]
        if predicted != actual:
            raise RuntimeError(f"model differs from actual native callback bytes for {name}: {len(predicted)} != {len(actual)}; predicted={predicted.hex()[:144]} actual={actual.hex()[:144]}")
        model_rows[name].update({"native_callback_bytes": len(actual), "model_exact": True,
                                 "native_callback_sha256": hashlib.sha256(actual).hexdigest()})
    return {"model_source_sha256": sha256(MODEL_RS), "path_source_sha256": sha256(MODEL_PATH),
            "source_of_model_inputs": "native ObjectStroke getter arrays and getter snapshots; selected drawable getSize/inverse-scale; captured redraw-entry state; captured screen profile",
            "comparisons": model_rows}


def main() -> None:
    ap = argparse.ArgumentParser()
    group = ap.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    args = ap.parse_args()
    pins = pin_inputs()
    env = os.environ.copy()
    env["PYTHONPATH"] = os.pathsep.join(str(ROOT / p) for p in ["scratch/apk-analysis-runtime/python", "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin", "conformance"])
    env["PATH"] = "/usr/bin"
    env["LD_LIBRARY_PATH"] = str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu")
    assert set(CASES) == EXPECTED_CASES, "aggregate case manifest changed"
    assert all(case["stroke_type"] == 0 for case in CASES.values()), "unsupported stroke type included"
    assert all(case.get("profile", PROFILE) == PROFILE for case in CASES.values())
    fixture = {"schema": "inkpen2-saved-adversarial-native-v2", "scope": "synthetic ObjectStroke setter inputs through actual native Drawing under explicit 1920x1080 host profile; not SDK deserialization/device defaults", "pins": pins, "cases": {}}
    base = BASE_CALLER.read_text()
    for name, case in CASES.items():
        # Every generated wrapper/input is ephemeral; only the intentional output fixture persists.
        with tempfile.TemporaryDirectory(prefix="inkpen2-native-") as td_raw:
            td = pathlib.Path(td_raw)
            probe = td / "probe.py"
            input_path = td / "input.json"
            probe.write_text(generated_probe(base, ROOT))
            input_path.write_text(json.dumps(case, separators=(",", ":")))
            cmd = [sys.executable, str(probe), "--print-capture", "--case", "line0_curve_short_history", "--advanced-setting", "9", "--width", str(PROFILE[0]), "--height", str(PROFILE[1]), "--input-json", str(input_path)]
            proc = subprocess.run(cmd, cwd=ROOT, env=env, text=True, capture_output=True)
        if proc.returncode:
            raise RuntimeError(f"native case {name} failed ({proc.returncode}):\n{proc.stderr[-2000:]}\n{proc.stdout[-1000:]}")
        result = json.loads(proc.stdout)
        assert_getter_capture(result)
        if name == "single_point":
            assert result["native_calls"]["factory_count"] == 0 and result["native_calls"]["selected_drawable"] is None
            assert result["native_calls"]["ObjectDrawing::drawObjectStroke_return"] == 0
            arrays = result["native_calls"]["source_ObjectStroke_arrays"]
            assert arrays["points"]["count"] == 1 and arrays["pressures"]["count"] == 1
            assert result["native_calls"]["screen_profile_requested_by_context"] == {"width": PROFILE[0], "height": PROFILE[1]}
            assert result["native_calls"]["screen_setup_reached"] is False and result["native_calls"]["pen_screen_resolution"] == []
        else:
            route = result["native_calls"]["route"]["drawing_selected_drawable_version"]
            assert route == "9", f"{name}: expected native V9 route, got {route}"
            assert result["native_calls"]["ObjectDrawing::drawObjectStroke_return"] == 1
            assert result["native_calls"]["selected_drawable_object_redraw_return"] == 1
        fixture["cases"][name] = result
        print(name, "rejected" if name == "single_point" else "V9", "records", sum(len(bytes.fromhex(e["delta_bytes"])) // 36 for e in result["callback_buffer_events"] if e["phase"] == "draw_object_stroke"))
    assert set(fixture["cases"]) == EXPECTED_CASES, "native aggregate omitted a case"
    fixture["numeric_model"] = rust_model_replay(fixture["cases"])
    assert set(fixture["numeric_model"]["comparisons"]) == EXPECTED_CASES, "model aggregate omitted a case"
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(fixture, indent=2, sort_keys=True) + "\n")
        print("wrote", FIXTURE)
    else:
        expected = json.loads(FIXTURE.read_text())
        if expected != fixture:
            raise SystemExit("saved-caller adversarial native/model fixture mismatch")
        print("verified all pinned saved-caller native cases and exact numeric streams")


if __name__ == "__main__":
    main()
