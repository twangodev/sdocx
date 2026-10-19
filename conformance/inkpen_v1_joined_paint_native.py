#!/usr/bin/env python3
"""Join the native InkPen V1 saved stream to RTV1 Draw and Mesa RGBA readback.

This wrapper applies asserted, in-memory adaptations to the pinned joined-paint
runner. It retains two synthetic Model inputs derived from the committed V1
numeric fixture: null settings/black and null settings/ARGB 0x80336699.
"""
import argparse
import hashlib
import json
import os
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/inkpen-v1-joined-paint-native.json"
NUMERIC = ROOT / "conformance/inkpen-v1-saved-native.json"
JOINED_RUNNER = ROOT / "conformance/inkpen_joined_paint_native.py"
CALLER = ROOT / "conformance/inkpen_saved_caller_native.py"
JOINED_RUNNER_SHA256 = "7a05db658343956af992823ee5a17622fa94a08c88152be9f39cee950eca4a79"
NUMERIC_SHA256 = "ade2af7ee7c7c07d4663151074932009f7def1a91ba4c14a4c55c294a3e32fb4"
HELPER_PINS = {
    "conformance/elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "conformance/fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "conformance/oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
    "conformance/inkpen_paint_native.py": "a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae",
}
DSO_PINS = {
    "Base": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "Composer": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "Drawing": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "FountainPen": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "Graphics": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "InkPen": "939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401",
    "Model": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "PenCommon": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "Renderer": "f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed",
    "libc++_shared": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
}


def pinned_sha(path, expected):
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    if expected and actual != expected:
        raise RuntimeError(f"pinned source changed: {path.name}: {actual}")
    return actual


def f32(bits):
    return struct.unpack("<f", bytes.fromhex(bits))[0]


def case_input(color_argb=None):
    pinned_sha(NUMERIC, NUMERIC_SHA256)
    oracle = json.loads(NUMERIC.read_text())
    row = oracle["cases"]["straight_curve_tool1_null_1440x2560"]
    source = row["input"]
    result = {
        "points": [[f32(x), f32(y)] for x, y in source["points_f32_bits"]],
        "size": f32(source["pen_size_f32_bits"]),
        "curve_enabled": bool(source["curve_enabled"]),
        "tool_type": source["tool_type"],
        "stroke_type": source["stroke_type"],
        "pressure": [f32(value) for value in source["pressure_f32_bits"]],
        "timestamps": source["timestamps_i32"],
        "tilt": [f32(value) for value in source["tilt_f32_bits"]],
        "orientation": [f32(value) for value in source["orientation_f32_bits"]],
    }
    if color_argb is not None:
        result["color_argb"] = color_argb
    return result, row


def replace_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"expected one {label} patch anchor, found {count}")
    return source.replace(old, new, 1)


def execute_native(case_id):
    runner_sha = pinned_sha(JOINED_RUNNER, JOINED_RUNNER_SHA256)
    caller_sha = hashlib.sha256(CALLER.read_bytes()).hexdigest()
    if caller_sha != "c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04":
        raise RuntimeError(f"pinned InkPen caller changed: {caller_sha}")
    color = 0x80336699 if case_id == "partial_alpha" else None
    for relative, expected in HELPER_PINS.items():
        pinned_sha(ROOT / relative, expected)
    library_dir = ROOT / "scratch/apk-analysis-native/arm64-v8a"
    for name, expected in DSO_PINS.items():
        filename = "libc++_shared.so" if name == "libc++_shared" else f"libSPen{name}.so"
        pinned_sha(library_dir / filename, expected)
    inputs, source_oracle = case_input(color)
    source = JOINED_RUNNER.read_text()
    patch_manifest = []

    def edit(old, new, label):
        nonlocal source
        source = replace_once(source, old, new, label)
        patch_manifest.append(label)

    edit("'--advanced-setting','empty','--width','1920','--height','1080'",
         "'--advanced-setting','null','--width','1440','--height','2560'", "native V1 route/profile")
    edit("vector_end-vector_begin!=312", "vector_end-vector_begin!=len(bytes.fromhex(ev['after_bytes']))", "size RT vector from native V1 callback stream")
    edit("row['size']==312 and row['sha256']==hashlib.sha256(bytes.fromhex(ev['after_bytes'])).hexdigest()",
         "row['size']==len(bytes.fromhex(ev['after_bytes'])) and row['sha256']==hashlib.sha256(bytes.fromhex(ev['after_bytes'])).hexdigest()",
         "size GPU upload assertion from native V1 callback stream")
    edit("caller_source=(ROOT/'conformance/inkpen_saved_caller_native.py').read_text()\n",
         "caller_source=(ROOT/'conformance/inkpen_saved_caller_native.py').read_text()\n"
         "case_input=json.loads(os.environ['INKPEN_CASE_JSON'])\n"
         "default_input=\"input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}\"\n"
         "if caller_source.count(default_input)!=1: raise RuntimeError('caller input anchor changed')\n"
         "caller_source=caller_source.replace(default_input,'input_case='+repr(case_input),1)\n"
         "if 'color_argb' in case_input:\n color_anchor=\"p.put(obj+32,'Q',impl)\"\n if caller_source.count(color_anchor)!=1: raise RuntimeError('post-Impl color setter anchor changed')\n caller_source=caller_source.replace(color_anchor,color_anchor+\"; p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,input_case['color_argb']])\",1)\n",
         "inject fixture source channels and optional post-Impl ARGB setter")
    edit("capture_dir=ROOT/'tmp/vector-parity-plan/inkpen-drawing-paint'",
         "capture_dir=ROOT/'tmp/vector-parity-plan/inkpen-v1-joined'", "isolate raw readback artifacts")
    edit("inkpen-v4-fbo4.rgba", "inkpen-v1-fbo4.rgba", "V1 FBO4 artifact label")
    edit("inkpen-v4-fbo3.rgba", "inkpen-v1-fbo3.rgba", "V1 FBO3 artifact label")
    edit("if sum(1 for i in range(0,len(image),4) if image[i:i+4]!=b'\\0\\0\\0\\0')!=171 or sum(1 for i in range(3,len(image),4) if image[i])!=171:raise RuntimeError('native RGBA output readback did not retain its 171-pixel coverage witness')",
         "if sum(1 for i in range(0,len(image),4) if image[i:i+4]!=b'\\0\\0\\0\\0')==0 or sum(1 for i in range(3,len(image),4) if image[i])==0:raise RuntimeError('native RGBA output readback is empty')",
         "require nonempty V1 RGBA output without V4-specific pixel count")
    edit("if sum(1 for i in range(0,len(fbo3rgba_raw),4) if fbo3rgba_raw[i:i+4]!=b'\\0\\0\\0\\0')!=171:raise RuntimeError('native RGBA mask readback did not retain its 171-pixel coverage witness')",
         "if sum(1 for i in range(0,len(fbo3rgba_raw),4) if fbo3rgba_raw[i:i+4]!=b'\\0\\0\\0\\0')==0:raise RuntimeError('native RGBA mask readback is empty')",
         "require nonempty V1 mask readback")
    edit("source_capture=getattr(m,'result',None)\n",
         "source_capture=getattr(m,'result',None)\nif source_capture is not None: source_capture['paint_color_calls']=m.paint_color_calls; source_capture['in_memory_patched_source_sha256']=patched_caller_source_sha256\n",
         "retain actual Pen::SetColor and RT::SetPenData calls and patch hash")
    edit("caller_source=caller_source.replace(return_anchor,return_body)\n",
         "caller_source=caller_source.replace(return_anchor,return_body)\npatched_caller_source_sha256=hashlib.sha256(caller_source.encode()).hexdigest()\n",
         "hash exact in-memory caller patch")
    edit("('case_id','profile','input','libraries','native_model_getters','route','screen_resolution','runtime_event_order','streams','native_calls')",
         "('case_id','profile','input','libraries','native_model_getters','route','screen_resolution','runtime_event_order','streams','native_calls','paint_color_calls','in_memory_patched_source_sha256')",
         "include native paint color setter calls")
    edit("'case_id':'inkpen_v4_black_1920x1080_to_128x64'",
         "'case_id':os.environ['INKPEN_CASE_ID']", "case id by control")
    edit("selected V4 RedrawPen and RT AddPoint stream", "selected V1 RedrawPen and RT AddPoint stream", "accurate V1 host-boundary wording")
    edit("Target dimensions 128x64 are a synthetic Mesa output surface; caller screen profile is explicit synthetic 1920x1080.",
         "Target dimensions 128x64 are a synthetic Mesa output surface; caller screen profile is explicit synthetic 1440x2560.",
         "accurate V1 profile wording")
    if "inkpen_v4_black_1920x1080_to_128x64" in source or "coverage witness" in source:
        raise RuntimeError("V4-only labels/assertions remain in patched runner")

    env = os.environ.copy()
    env.update({
        "INKPEN_REPO_ROOT": str(ROOT),
        "INKPEN_CASE_ID": "inkpen_v1_partial_alpha_1440x2560" if color is not None else "inkpen_v1_null_black_1440x2560",
        "INKPEN_CASE_JSON": json.dumps(inputs, separators=(",", ":")),
    })
    original_argv = sys.argv
    original_env = {key: os.environ.get(key) for key in ("INKPEN_REPO_ROOT", "INKPEN_CASE_ID", "INKPEN_CASE_JSON")}
    sys.argv = [str(JOINED_RUNNER), "--print-capture"]
    os.environ.update(env)
    # The original script derives ROOT from __file__; compile with its pinned path.
    output_capture = __import__("io").StringIO()
    try:
        with __import__("contextlib").redirect_stdout(output_capture):
            exec(compile(source, str(JOINED_RUNNER), "exec"), {"__name__": "__main__", "__file__": str(JOINED_RUNNER), "os": os})
    finally:
        sys.argv = original_argv
        for key, value in original_env.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    output = output_capture.getvalue()
    marker = '\n{\n  "case_id":'
    if marker not in output:
        raise RuntimeError("joined runner did not emit a parseable result object")
    result = json.loads(output[output.index(marker) + 1:])
    result["target"]["readback"]["row_order"] = "glReadPixels origin is lower-left; captured bytes retain native row order without vertical flipping"
    result["wrapper_provenance"] = {
        "wrapper_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "pinned_joined_runner_sha256": runner_sha,
        "pinned_saved_caller_sha256": caller_sha,
        "pinned_helper_sha256": HELPER_PINS,
        "pinned_dso_sha256": DSO_PINS,
        "case_input_from_numeric_oracle": "straight_curve_tool1_null_1440x2560",
        "numeric_fixture_sha256": NUMERIC_SHA256,
        "patch_manifest": patch_manifest,
        "patched_runner_sha256": hashlib.sha256(source.encode()).hexdigest(),
        "execution_boundary": "Existing joined-paint runner executes same selected RT initialization, native SendDataToGPU queue task, Update(false), Draw, Renderer, native shaders and host Mesa readback. Input Model setters are synthetic; no serialized-note admission/device parity claim.",
    }
    outputcase = result["case_id"]
    expected_id = "inkpen_v1_partial_alpha_1440x2560" if color is not None else "inkpen_v1_null_black_1440x2560"
    if outputcase != expected_id:
        raise RuntimeError(f"wrong joined capture case id: {outputcase}")
    if result["source_caller"]["route"]["selected_drawable_class"] != "SPen::InkPenStrokeDrawableGLV1":
        raise RuntimeError("native Drawing did not select V1")
    if result["source_caller"]["route"]["same_rt_id"] != "rt0" or not result["factory_path"]["same_selected_rt"]:
        raise RuntimeError("Drawing and paint did not retain the same selected RT")
    stream = result["source_caller"]["streams"]["draw_object_stroke"]
    if stream["values_f32_bits"] != source_oracle["streams"]["draw_object_stroke"]["values_f32_bits"]:
        raise RuntimeError("native V1 geometry differs from committed numeric fixture")
    if stream["sha256"] != source_oracle["streams"]["draw_object_stroke"]["sha256"]:
        raise RuntimeError("native V1 stream hash differs from committed numeric fixture")
    handoff = result["factory_path"]["native_vector_handoff"]
    if handoff["vector_sha256_at_update"] != stream["sha256"] or handoff["stream_sha256"] != stream["sha256"]:
        raise RuntimeError("same-RT native update did not consume the expected V1 stream")
    color_value = 0x80336699 if color is not None else 0xff000000
    snapshots = result["source_caller"]["native_calls"]["ObjectStroke_getter_snapshots"]
    if any(row["color_argb"] != color_value for row in snapshots):
        raise RuntimeError("native ObjectStroke GetColor did not retain the requested ARGB")
    color_calls = result["source_caller"]["paint_color_calls"]
    setdata = [row for row in color_calls if row["method"] == "SPen::PenDrawableRT::SetPenData(float, int)"]
    if not setdata or any(int(row["argb_u32"], 16) != color_value for row in setdata):
        raise RuntimeError("native RT SetPenData did not receive expected ARGB")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    group.add_argument("--capture-case", choices=("black", "partial_alpha"))
    args = parser.parse_args()
    # Guard the exact upstream fixture and original runner sources before either mode.
    pinned_sha(JOINED_RUNNER, JOINED_RUNNER_SHA256)
    pinned_sha(CALLER, "c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04")
    if args.capture_case:
        result = execute_native(args.capture_case)
        print(json.dumps(result, sort_keys=True))
        return
    cases = {}
    for case_id in ("black", "partial_alpha"):
        child = subprocess.run([sys.executable, str(Path(__file__).resolve()), "--capture-case", case_id],
                               cwd=ROOT, env=os.environ.copy(), text=True, capture_output=True, check=True)
        cases[case_id] = json.loads(child.stdout)
    black = cases["black"]
    partial = cases["partial_alpha"]
    if black["source_caller"]["streams"]["draw_object_stroke"]["values_f32_bits"] != partial["source_caller"]["streams"]["draw_object_stroke"]["values_f32_bits"]:
        raise RuntimeError("ARGB control changed V1 geometry")
    if black["target"]["readback"]["fbo4_rgba8_sha256"] == partial["target"]["readback"]["fbo4_rgba8_sha256"]:
        raise RuntimeError("partial-alpha control did not change the native RGBA target")
    payload = {
        "schema": "sdocx-inkpen-v1-joined-paint-native-v1",
        "wrapper_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "cases": cases,
    }
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    else:
        if not FIXTURE.exists() or json.loads(FIXTURE.read_text()) != payload:
            raise SystemExit("native InkPen V1 joined paint fixture mismatch")
        print(f"verified {len(cases)} V1 joined paint cases")


if __name__ == "__main__":
    main()
