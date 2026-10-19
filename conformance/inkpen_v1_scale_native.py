#!/usr/bin/env python3
"""Replay InkPen V1 Draw at multiple native target scales and identity controls."""
import argparse, base64, contextlib, hashlib, json, os, subprocess, sys, zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/inkpen-v1-scale-native.json"
UPSTREAM = ROOT / "conformance/inkpen_v1_joined_paint_native.py"
UPSTREAM_SHA = "67e1fac4c3d9799f9c4dfc0a2d15e93399c83c186e50119b824af96afe7dfb01"
JOINED_SHA = "7a05db658343956af992823ee5a17622fa94a08c88152be9f39cee950eca4a79"
NUMERIC_SHA = "ade2af7ee7c7c07d4663151074932009f7def1a91ba4c14a4c55c294a3e32fb4"
PAINT_FIXTURE_SHA = "07f61f8b8a01304a5929da875ad2cce18b8cece6ee3406565f686ebcef7c3233"
CALLER_SHA = "c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04"
HELPERS = {
    "conformance/elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "conformance/fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "conformance/oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
    "conformance/inkpen_paint_native.py": "a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae",
}
DSOS = {
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
UPLOADS = {
    "init": "6b0b87ae7ed17d448d3c052f787a549360b705521bd41576f4943dd63b784c75",
    "update": "121ec3fb8daf520affed22f475ac2e90931e48b4d49387fb658f51ffa50ced31",
}
STREAM_SHA = "729db51fe3a6d6d8062a3e5cc99273f5023e971a517468133368d01ae124103c"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pin(path, expected):
    got = sha(path)
    if got != expected:
        raise RuntimeError(f"pinned input changed: {path.relative_to(ROOT)}: {got}")


def preflight():
    for relative, expected in {
        "conformance/inkpen_v1_joined_paint_native.py": UPSTREAM_SHA,
        "conformance/inkpen_joined_paint_native.py": JOINED_SHA,
        "conformance/inkpen_saved_caller_native.py": CALLER_SHA,
        "conformance/inkpen-v1-saved-native.json": NUMERIC_SHA,
        "conformance/inkpen-v1-joined-paint-native.json": PAINT_FIXTURE_SHA,
        **HELPERS,
    }.items():
        pin(ROOT / relative, expected)
    library_dir = ROOT / "scratch/apk-analysis-native/arm64-v8a"
    for name, expected in DSOS.items():
        pin(library_dir / ("libc++_shared.so" if name == "libc++_shared" else f"libSPen{name}.so"), expected)


def replace_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one patch anchor, found {count}")
    return source.replace(old, new, 1)


def rgba_bytes(result, fbo):
    key = "fbo4_rgba8_zlib_base64" if fbo == 4 else "fbo3_rgba8_zlib_base64"
    return zlib.decompress(base64.b64decode(result["target"]["readback"][key]))


def metrics(raw):
    alpha = raw[3::4]
    peak = max(alpha, default=0)
    return {
        "support_pixels": sum(x > 0 for x in alpha),
        "full_coverage_pixels": sum(x == peak for x in alpha) if peak else 0,
        "edge_pixels": sum(0 < x < peak for x in alpha),
        "maximum_alpha": peak,
    }


def capture(case, scale, transform):
    source = UPSTREAM.read_text()
    guard = '    if "inkpen_v4_black_1920x1080_to_128x64" in source or "coverage witness" in source:\n'
    old_upload_guard = "if not any(row['size']==48 and row['sha256']=='6b0b87ae7ed17d448d3c052f787a549360b705521bd41576f4943dd63b784c75' for row in init_uploads) or not any(row['size']==48 and row['sha256']=='121ec3fb8daf520affed22f475ac2e90931e48b4d49387fb658f51ffa50ced31' for row in update_uploads):raise RuntimeError('native Init/Update composite buffers are missing')"
    raw_upload_guard = "if not any(row['size']==48 and row.get('raw_hex') for row in init_uploads) or not any(row['size']==48 and row.get('raw_hex') for row in update_uploads):raise RuntimeError('native composite VBO byte capture missing')"
    init_old = "state.setdefault('early_gl_buffer_data',[]).append({'target':target,'buffer':bound.value,'size':size,'sha256':hashlib.sha256(raw).hexdigest()})"
    init_new = "state.setdefault('early_gl_buffer_data',[]).append({'target':target,'buffer':bound.value,'size':size,'sha256':hashlib.sha256(raw).hexdigest(),'raw_hex':raw.hex()})"
    update_old = "state.setdefault('mesa_buffer_data',[]).append({'target':target,'buffer':bound.value,'size':size,'sha256':hashlib.sha256(raw).hexdigest()})"
    update_new = "state.setdefault('mesa_buffer_data',[]).append({'target':target,'buffer':bound.value,'size':size,'sha256':hashlib.sha256(raw).hexdigest(),'raw_hex':raw.hex()})"
    injection = (
        '    scale=int(os.environ["INKPEN_CAPTURE_SCALE"])\n'
        '    transform=int(os.environ["INKPEN_DRAW_TRANSFORM"])\n'
        '    edit("W,H=128,64", f"W,H={128*scale},{64*scale}", "fresh FBO and native viewport scale")\n'
        '    edit("projection=(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)", f"projection=({float(transform)},0,0,0,0,{float(transform)},0,0,0,0,1,0,0,0,0,1)", "native Draw input transform")\n'
        f'    if scale != transform: edit({old_upload_guard!r}, {raw_upload_guard!r}, "identity-transform VBO guard")\n'
        f'    edit({init_old!r}, {init_new!r}, "capture native Init VBO bytes")\n'
        f'    edit({update_old!r}, {update_new!r}, "capture native Update VBO bytes")\n'
        + guard
    )
    source = replace_once(source, guard, injection, "scale patch location")
    old_argv = sys.argv
    old_env = {k: os.environ.get(k) for k in ("INKPEN_CAPTURE_SCALE", "INKPEN_DRAW_TRANSFORM")}
    sys.argv = [str(UPSTREAM), "--capture-case", case]
    os.environ["INKPEN_CAPTURE_SCALE"] = str(scale)
    os.environ["INKPEN_DRAW_TRANSFORM"] = str(transform)
    output = __import__("io").StringIO()
    try:
        with contextlib.redirect_stdout(output):
            exec(compile(source, str(UPSTREAM), "exec"), {
                "__name__": "__main__", "__file__": str(UPSTREAM), "os": os,
            })
    finally:
        sys.argv = old_argv
        for key, value in old_env.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    result = json.loads(output.getvalue())
    result["scale_probe"] = {
        "case": case,
        "scale": scale,
        "draw_transform_diagonal": [transform, transform, 1, 1],
        "target_dimensions": [128 * scale, 64 * scale],
        "projection_uniform_uploads": [r for r in result["target"]["native_shader_uniform_trace"] if r[0] == "glUniformMatrix4fv"],
        "native_composite_48byte_uploads": {
            "init": [r for r in result["factory_path"]["renderer_init_buffer_data"] if r["size"] == 48],
            "update": [r for r in result["factory_path"]["native_update_buffer_data"] if r["size"] == 48],
        },
        "runner_sha256": sha(Path(__file__).resolve()),
        "upstream_wrapper_sha256": UPSTREAM_SHA,
        "patches": [
            "exact-counted insertion at pinned joined-wrapper scale guard",
            "fresh target dimensions W,H = 128*scale,64*scale",
            "native Draw matrix diagonal transform",
            "raw bytes captured for native Init/Update GLES uploads",
            "strict known 48-byte VBO hashes retained for scale-matched controls",
        ],
        "scope": "native synthetic Drawing and same InkPen RT saved replay/Draw; host Draw transform and fresh Mesa FBOs; no device/export/PDF parity claim",
    }
    verify(result, case, scale, transform)
    return result


def verify(result, case, scale, transform):
    width, height = 128 * scale, 64 * scale
    source, target = result["source_caller"], result["target"]
    readback = target["readback"]
    if source["route"]["selected_drawable_class"] != "SPen::InkPenStrokeDrawableGLV1":
        raise RuntimeError("selected saved drawable is not InkPen V1")
    if not result["factory_path"]["same_selected_rt"] or source["route"]["same_rt_id"] != "rt0":
        raise RuntimeError("Drawing and paint do not retain the same selected RT")
    stream = source["streams"]["draw_object_stroke"]
    if stream["float_count"] != 75 or stream["sha256"] != STREAM_SHA:
        raise RuntimeError("native V1 stream does not match the pinned 75-lane oracle")
    if result["factory_path"]["native_vector_handoff"]["vector_sha256_at_update"] != STREAM_SHA:
        raise RuntimeError("native Update did not consume the expected saved stream")
    if target["viewport"] != [0, 0, width, height] or len(target["draws"]) != 2:
        raise RuntimeError("scaled target viewport/draw count mismatch")
    if any(d["viewport"] != [0, 0, width, height] or d["gl_error_after"] != 0 for d in target["draws"]):
        raise RuntimeError("native draw viewport or GL error mismatch")
    for fbo, read in ((4, readback["fbo4_read"]), (3, readback["fbo3_read"])):
        raw = rgba_bytes(result, fbo)
        if len(raw) != width * height * 4 or read["dimensions"] != [width, height]:
            raise RuntimeError("RGBA payload dimensions mismatch")
        if hashlib.sha256(raw).hexdigest() != readback[f"fbo{fbo}_rgba8_sha256"]:
            raise RuntimeError(f"FBO{fbo} full RGBA payload hash mismatch")
        if read["framebuffer_status"] != 0x8CD5 or read["preexisting_gl_error"] or read["gl_error_after_rgba_read"]:
            raise RuntimeError("native framebuffer/readback validation failed")
    uploads = result["scale_probe"]["native_composite_48byte_uploads"]
    for which in ("init", "update"):
        row = uploads[which][0]
        raw = bytes.fromhex(row["raw_hex"])
        if len(raw) != 48 or hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise RuntimeError("native composite VBO upload bytes/hash mismatch")
        if scale == transform and row["sha256"] != UPLOADS[which]:
            raise RuntimeError("scale-matched run changed native 48-byte VBO upload")
    mats = result["scale_probe"]["projection_uniform_uploads"]
    if len(mats) != 1:
        raise RuntimeError("expected one captured native projection matrix upload")
    matrix = [float.fromhex(x) for x in mats[0][4]]
    if matrix[0] != transform / (64.0 * scale) or matrix[5] != transform / (32.0 * scale):
        raise RuntimeError("projection uniform does not match target/transform scale")
    if matrix[12:14] != [-1.0, -1.0]:
        raise RuntimeError("projection matrix translation changed")
    if not any("fwidth(dist)" in str(r) for r in target["native_shader_uniform_trace"] if r[0] == "glShaderSource"):
        raise RuntimeError("native InkPen fragment shader fwidth source was not captured")
    color = 0x80336699 if case == "partial_alpha" else 0xFF000000
    if any(row["color_argb"] != color for row in source["native_calls"]["ObjectStroke_getter_snapshots"]):
        raise RuntimeError("native source ARGB getter did not match control")
    setdata = [r for r in source["paint_color_calls"] if r["method"] == "SPen::PenDrawableRT::SetPenData(float, int)"]
    if not setdata or any(int(row["argb_u32"], 16) != color for row in setdata):
        raise RuntimeError("native saved ARGB was not forwarded to RTV1")
    field = metrics(rgba_bytes(result, 4))
    result["scale_probe"]["rgba_coverage_metrics"] = field
    expected_alpha = 128 if case == "partial_alpha" else 255
    if field["maximum_alpha"] != expected_alpha:
        raise RuntimeError("native RGBA maximum alpha does not match ARGB control")
    if scale == transform and field["support_pixels"] != {1: 175, 2: 700, 4: 2808}[scale]:
        raise RuntimeError("native scale coverage witness changed")


def specs():
    return (
        ("black_1x", "black", 1, 1),
        ("black_2x", "black", 2, 2),
        ("black_4x", "black", 4, 4),
        ("partial_alpha_1x", "partial_alpha", 1, 1),
        ("partial_alpha_2x", "partial_alpha", 2, 2),
        ("partial_alpha_4x", "partial_alpha", 4, 4),
        ("identity_target_2x", "black", 2, 1),
        ("identity_target_4x", "black", 4, 1),
    )


def build_fixture():
    preflight()
    cases = {}
    for case_id, color, scale, transform in specs():
        cmd = [sys.executable, str(Path(__file__).resolve()), "--capture-case", color, "--scale", str(scale), "--draw-transform", str(transform)]
        child = subprocess.run(cmd, cwd=ROOT, env=os.environ.copy(), text=True, capture_output=True, check=True)
        result = json.loads(child.stdout)
        result["scale_probe"]["case_id"] = case_id
        cases[case_id] = result
    baseline = rgba_bytes(cases["black_1x"], 4)
    paint_baseline = json.loads((ROOT / "conformance/inkpen-v1-joined-paint-native.json").read_text())
    for case in ("black", "partial_alpha"):
        if cases[f"{case}_1x"]["target"]["readback"]["fbo4_rgba8_sha256"] != paint_baseline["cases"][case]["target"]["readback"]["fbo4_rgba8_sha256"]:
            raise RuntimeError(f"1x {case} output differs from pinned joined-paint baseline")
    for scale in (2, 4):
        high = rgba_bytes(cases[f"identity_target_{scale}x"], 4)
        width = 128 * scale
        crop = b"".join(high[y * width * 4:y * width * 4 + 128 * 4] for y in range(64))
        if crop != baseline or cases[f"identity_target_{scale}x"]["scale_probe"]["rgba_coverage_metrics"]["support_pixels"] != 175:
            raise RuntimeError("identity-transform resize changed 1x page-space pixels")
    for scale in (1, 2, 4):
        if cases[f"black_{scale}x"]["target"]["readback"]["fbo4_rgba8_sha256"] == cases[f"partial_alpha_{scale}x"]["target"]["readback"]["fbo4_rgba8_sha256"]:
            raise RuntimeError("partial ARGB control did not change the native target")
        if rgba_bytes(cases[f"black_{scale}x"], 3) != rgba_bytes(cases[f"partial_alpha_{scale}x"], 3):
            raise RuntimeError("saved ARGB changed the native alpha-mask FBO")
    return {
        "schema": "sdocx-inkpen-v1-scale-native-v1",
        "runner_sha256": sha(Path(__file__).resolve()),
        "source_pins": {
            "upstream_wrapper_sha256": UPSTREAM_SHA,
            "joined_runner_sha256": JOINED_SHA,
            "numeric_fixture_sha256": NUMERIC_SHA,
            "saved_caller_sha256": CALLER_SHA,
            "helper_sha256": HELPERS,
            "dso_sha256": DSOS,
        },
        "interpretation": "Eight native V1 RTV1 draws use fresh Mesa mask/output FBOs. Six scale-matched cases rerun Draw with explicit host transform diag(1/2/4); two identity-transform controls enlarge only the target and retain the 1x page-space pixel crop. Native stream, shaders, uploads, uniforms, and full RGBA payloads are retained. Inputs are synthetic; no Android-device or PDF parity is claimed.",
        "cases": cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    group.add_argument("--capture-case", choices=("black", "partial_alpha"))
    parser.add_argument("--scale", type=int, choices=(1, 2, 4))
    parser.add_argument("--draw-transform", type=int, choices=(1, 2, 4))
    args = parser.parse_args()
    preflight()
    if args.capture_case:
        if args.scale is None or args.draw_transform is None:
            parser.error("--capture-case requires --scale and --draw-transform")
        print(json.dumps(capture(args.capture_case, args.scale, args.draw_transform), sort_keys=True))
        return
    if args.scale is not None or args.draw_transform is not None:
        parser.error("--scale/--draw-transform require --capture-case")
    payload = build_fixture()
    if args.write_fixture:
        temp = FIXTURE.with_suffix(FIXTURE.suffix + ".tmp")
        temp.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
        os.replace(temp, FIXTURE)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif not FIXTURE.exists() or json.loads(FIXTURE.read_text()) != payload:
        raise SystemExit("native InkPen scale fixture mismatch")
    else:
        print(f"verified {len(payload['cases'])} V1 scale cases")


if __name__ == "__main__":
    main()
