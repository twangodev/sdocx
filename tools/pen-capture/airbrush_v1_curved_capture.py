#!/usr/bin/env python3
"""Capture one curved AirBrush V1 Model/Drawing/Mesa control to stdout."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
runner = root / "tmp/vector-parity-plan/pen-family-audit/airbrush-v1-short-native/frame-capture.py"
manifest = json.loads((root / "tools/pen-capture/airbrush_v1_curved_prerequisites.json").read_text())
for path, expected in manifest.items():
    actual = hashlib.sha256((root / path).read_bytes()).hexdigest()
    if actual != expected:
        raise RuntimeError(f"native prerequisite hash mismatch: {path}")
source = runner.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == "d38a83f3cf71b228f58654c08a135d3f91d3bda0349df1d6014926752dc3b73e"
source = source.replace("'points':[_probe_start]*n", "'points':[[120,120],[120.125,120.25],[122,121],[124,118],[124.0625,118],[126,123]]")
source = source.replace("'pressure':[.3]*n", "'pressure':[0,.1,.9,.3,1,.5]")
source = source.replace("'timestamps':[300+15*i for i in range(n)]", "'timestamps':[300,301,310,450,451,900]")
source = source.replace("'tilt':[.12]*n", "'tilt':[0,.1,.5,.2,.7,.3]")
source = source.replace("'orientation':[.2]*n", "'orientation':[0,.2,.9,.4,1.2,.6]")
env = os.environ.copy()
env.pop("PYTHONOPTIMIZE", None)
env["AB_N"] = "6"
env["PYTHONPATH"] = ":".join(str(root / p) for p in ("scratch/apk-analysis-runtime/python", "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin", "tmp/conformance-cleanup/native-capture-a241793/conformance"))
env["LD_LIBRARY_PATH"] = str(root / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu")
with tempfile.NamedTemporaryFile(mode="w", suffix=".py") as script:
    script.write(source)
    script.flush()
    subprocess.run([sys.executable, script.name, "--single-case", "--print-capture", "--advanced-mode", "unset", "--width", "256", "--height", "256", "--source-size", "8.75", "--color-argb", "0x8044ee11"], cwd=root, env=env, check=True)
