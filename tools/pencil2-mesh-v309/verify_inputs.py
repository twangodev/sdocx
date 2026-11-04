"""Verify retained native inputs without loading or printing note content."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--native-local", action="store_true", help="also check direct local native capture adapters")
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
manifest = json.loads((root / "tools/pencil2-mesh-v309/input-manifest.json").read_text())
optional = set(manifest["native_local_paths"])
errors = []
checked = 0
for path, expected in manifest["sha256"].items():
    if path in optional and not args.native_local:
        continue
    checked += 1
    file = root / path
    if not file.is_file():
        errors.append(f"missing: {path}")
    elif hashlib.sha256(file.read_bytes()).hexdigest() != expected:
        errors.append(f"changed: {path}")
if errors:
    raise SystemExit("\n".join(errors))
print(f"Verified {checked} retained native inputs; local adapters={args.native_local}")
