"""Capture a synthetic OilBrush3 clear=0 caller-page control on host Mesa.

Requires the pinned scratch/oilbrush3-public-drawing-v17 runner, its preflight
and native-library dependencies, and Unicorn 2.1.4; this is not a self-contained
reproduction environment. Run without Python -O so capture assertions execute.
The retained frames bound one lifecycle, not saved-note, device, general page
initialization, or public-dispatch parity. Draw uniforms are not retained here;
Rust derives them using the separate native matrix and feedback controls.
"""

import pathlib
import os
import tempfile
import hashlib
import json
import subprocess
import sys
import base64
import zlib

root = pathlib.Path(__file__).resolve().parents[2]
src = root / "scratch/oilbrush3-public-drawing-v17"
runner_path = src / "oilbrush3_public_drawing_capture.py"
manifest_path = src / "preflight.json"
canvas_path = src / "oilbrush3_canvas_native.py"
for path, expected in (
    (runner_path, "4d64a429802111757167a2878c941ff51c69f16320c5da9dc138a267276198cf"),
    (manifest_path, "2c9304fd6e7990cc5e8657e5698817b6405b2d428ac82aeb00c5e8b2320f179c"),
    (canvas_path, "9714aa7237adfbeef6750a698f98f1dba7010ff698203f92c6569833b4422bbd"),
):
    assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, path
raw = runner_path.read_text()
manifest = json.loads(manifest_path.read_text())
needle = " bitmap_record=target_bitmap_record"
injection = """\n bind=ip.cfunc(_gl,'glBindTexture',None,[ctypes.c_uint,ctypes.c_uint])\n active=_oil_gl_int(0x84E0); _gl_active_texture(0x84C0); previous=_oil_gl_int(0x8069); texture=target_bitmap_record['native_texture_object']['gl_texture_name']; bind(0x0DE1,texture)\n data=ctypes.create_string_buffer(bytes([41,83,127,255])*1920*1080); _gl_tex_sub_image_2d(0x0DE1,0,0,0,1920,1080,0x1908,0x1401,ctypes.cast(data,ctypes.c_void_p))\n bind(0x0DE1,previous); _gl_active_texture(active)\n gen=ip.cfunc(_gl,'glGenFramebuffers',None,[ctypes.c_int,ctypes.POINTER(ctypes.c_uint)]); fb=ctypes.c_uint(); gen(1,ctypes.byref(fb))\n bindfb=ip.cfunc(_gl,'glBindFramebuffer',None,[ctypes.c_uint,ctypes.c_uint]); attach=ip.cfunc(_gl,'glFramebufferTexture2D',None,[ctypes.c_uint,ctypes.c_uint,ctypes.c_uint,ctypes.c_uint,ctypes.c_int]); delete=ip.cfunc(_gl,'glDeleteFramebuffers',None,[ctypes.c_int,ctypes.POINTER(ctypes.c_uint)])\n olddraw=_oil_gl_int(0x8CA6); oldread=_oil_gl_int(0x8CAA); bindfb(0x8D40,fb.value); attach(0x8D40,0x8CE0,0x0DE1,texture,0)\n injected=_oil_draw_readback(fb.value); _console_print('INJECTED='+json.dumps(injected),file=sys.stderr)\n bindfb(0x8CA9,olddraw); bindfb(0x8CA8,oldread); delete(1,ctypes.byref(fb))\n"""
assert raw.count(needle) == 1
raw = raw.replace(needle, injection + needle)
with tempfile.TemporaryDirectory(prefix="oil-init-") as td:
    p = pathlib.Path(td)
    (p / "oilbrush3_canvas_native.py").write_bytes(
        (src / "oilbrush3_canvas_native.py").read_bytes()
    )
    assert raw.count("'clear':1,'screen_profile'") == 1
    assert (
        raw.count("[p.drawing,target_bitmap,obj,draw_rect,matrix,1],limit=5000000)")
        == 1
    )
    for clear in (0, 1):
        text = raw.replace(
            "'clear':1,'screen_profile'", f"'clear':{clear},'screen_profile'"
        ).replace(
            "[p.drawing,target_bitmap,obj,draw_rect,matrix,1],limit=5000000)",
            f"[p.drawing,target_bitmap,obj,draw_rect,matrix,{clear}],limit=5000000)",
        )
        run = p / "oilbrush3_public_drawing_capture.py"
        run.write_text(text)
        m = dict(manifest)
        m["runner_sha256"] = hashlib.sha256(run.read_bytes()).hexdigest()
        mp = p / "preflight.json"
        mp.write_text(json.dumps(m))
        q = subprocess.run(
            [
                sys.executable,
                str(run),
                "--preflight-manifest",
                str(mp),
                "--preflight-sha256",
                hashlib.sha256(mp.read_bytes()).hexdigest(),
                "--compact",
                "--failure-json",
                str(p / "failure.json"),
            ],
            cwd=root,
            capture_output=True,
            text=True,
        )
        print("case", clear, "exit", q.returncode, flush=True)
        if q.returncode:
            raise RuntimeError(q.stderr[-3000:])
        injected = json.loads(
            next(
                line[9:]
                for line in q.stderr.splitlines()
                if line.startswith("INJECTED=")
            )
        )
        injected_bytes = zlib.decompress(base64.b64decode(injected["payload_base64"]))
        assert (
            injected["sha256"]
            == "665324f95724c2cd95e5c4898d70ece7bc1351ebf48f2e8c91b6180c80f8066f"
        )
        assert len(injected_bytes) == 1920 * 1080 * 4
        c = json.loads(q.stdout)
        first = c["mesa_draw_observations"][0]["before"]
        assert first["viewport"] == [0, 0, 1920, 1080]
        assert (
            first["pixels"]["texture_id"]
            == c["mesa_native_texture_objects"][0]["gl_texture_name"]
        )
        pix = first["pixels"]
        b = zlib.decompress(base64.b64decode(pix["payload_base64"]))
        expected = (
            (
                1920 * 1080,
                "665324f95724c2cd95e5c4898d70ece7bc1351ebf48f2e8c91b6180c80f8066f",
                bytes([41, 83, 127, 255]),
            )
            if clear == 0
            else (
                0,
                "788ae0147bdf979a6575938ca2d7d4403788588f7be2010f03776c968fd1ab49",
                bytes(4),
            )
        )
        assert (
            len(b) == 1920 * 1080 * 4
            and pix["nonzero_pixels"] == expected[0]
            and pix["sha256"] == expected[1]
        )
        assert b == (injected_bytes if clear == 0 else bytes(len(b)))
        assert (
            c["native_execution_evidence"]["drawing_return"] == 1
            and c["native_execution_evidence"]["queue_end_error_observation"][
                "actual_mesa_error"
            ]
            == 0
        )
        if clear == 0:
            pages = [
                d for d in c["mesa_draw_observations"]
                if d["before"]["viewport"] == [0, 0, 1920, 1080]
            ]
            assert len(pages) == 15
            frames = [
                zlib.decompress(base64.b64decode(d["after"]["pixels"]["payload_base64"]))
                for d in pages
            ]
            assert all(len(frame) == 1920 * 1080 * 4 for frame in frames)
            fixture_bytes = zlib.compress(b"".join(frames), 9)
            receipt = {
                "scope": "Synthetic host Mesa Drawing clear=0 with caller-injected uniform opaque full page; not device or saved-note parity",
                "initial_sha256": hashlib.sha256(injected_bytes).hexdigest(),
                "prefix_sha256": [hashlib.sha256(frame).hexdigest() for frame in frames],
                "runner_sha256": hashlib.sha256(runner_path.read_bytes()).hexdigest(),
                "preflight_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
                "native_hashes": c["native_hashes"],
                "drawing_return": 1,
                "queue_end_gl_error": 0,
            }
        print("caller page verified, clear =", clear, flush=True)

# Publish only after both controls passed. Each replacement is atomic, although
# the two files are not a single atomic transaction.
dest = root / "conformance/oilbrush3-overlap"
dest.mkdir(exist_ok=True)
for name, data in (
    ("prefixes.zlib", fixture_bytes),
    ("receipt.json", (json.dumps(receipt, indent=2) + "\n").encode()),
):
    with tempfile.NamedTemporaryFile(dir=dest, prefix=".capture-", delete=False) as out:
        try:
            out.write(data)
            out.flush()
            os.replace(out.name, dest / name)
        finally:
            pathlib.Path(out.name).unlink(missing_ok=True)
