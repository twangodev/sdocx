"""Capture the GLV1 source-alpha pair using the pinned historical paint harness.

Restore conformance from 2ffac9f^ into a project-shaped temporary directory,
copy AGENTS.md there, symlink scratch, and pass that directory as the argument.
"""
import argparse
import base64
import hashlib
import json
import subprocess
import sys
import zlib
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("root", type=Path)
root = parser.parse_args().root.resolve()
runner = root / "conformance/chinesebrush_paint_native.py"
original = runner.read_text()
assert hashlib.sha256(original.encode()).hexdigest() == "0377f7de9c48e7c79fc596603128d49bff3a7427ce59e6052db14a53d8cf342a"
case = json.loads((root / "conformance/chinesebrush-source-native.json").read_text())["cases"]["curve-v1"]
case_sha = hashlib.sha256(json.dumps(case, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
source = (original.replace("0dd9a905a6c5b61cec192c5e8063d7c424ceec824bc1c639eef75e5c41c065d1", case_sha)
          .replace("curve-v3", "curve-v1").replace("GLV3", "GLV1")
          .replace("b'3;\\0'", "b'1;\\0'").replace("'3;'", "'1;'"))
anchor = " return case\ndef preflight():"
assert source.count(anchor) == 1
source = source.replace(anchor, " case['input']['color_argb_u32']='COLOR'\n case['input']['native_model_getter_snapshot']['color_argb_u32']='COLOR'\n return case\ndef preflight():")
assertion = "    if color['rt_plus44_bytes']!='d9d8d83ededd5d3f908f0f3f0000803f': raise RuntimeError('native queued color does not match canonical ChineseBrush ARGB fixture')\n"
assert source.count(assertion) == 1
source = source.replace(assertion, "")
frames = []
try:
    for color in ("0xff6cdd8f", "0x406cdd8f"):
        runner.write_text(source.replace("0xff6cdd8f", color).replace("COLOR", color))
        subprocess.run([sys.executable, str(runner), "--write-fixture"], check=True)
        report = json.loads((root / "conformance/chinesebrush-paint-native.json").read_text())
        frame = zlib.decompress(base64.b64decode(report["paint_readback"]["full_rgba8_bottom_left_first_zlib_base64"]))
        frames.append((hashlib.sha256(frame).hexdigest(), report["native_draw"]["producer_vector_sha256"], report["native_draw"]["same_rt_input_color_join"]["uniform_bytes"]))
    assert frames[0][:2] == frames[1][:2] and frames[0][2] != frames[1][2], frames
    print(json.dumps(frames, indent=2))
finally:
    runner.write_text(original)
