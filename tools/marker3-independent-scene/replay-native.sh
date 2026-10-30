#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
work=$(mktemp -d /tmp/sdocx-marker3-independent.XXXXXX)
trap 'rm -rf "$work"' EXIT
python_bin=${PYTHON:-.venv/bin/python}
"$python_bin" "$root/tools/marker3-terminal-size85/capture.py" "$work/base"
case_dir="$work/base/phase2-tool1"
patch -s -o "$case_dir/dual-caller.py" "$case_dir/caller.py" "$root/tools/marker3-independent-scene/caller.patch"
sed "s|/tmp/marker3-dual-proto/phase2-tool1|$case_dir|g" "$root/tools/marker3-independent-scene/join.patch" > "$work/join.patch"
patch -s -o "$case_dir/dual-join.py" "$case_dir/join.py" "$work/join.patch"
patch -s -o "$case_dir/dual-reverse.py" "$case_dir/dual-join.py" "$root/tools/marker3-independent-scene/reverse.patch"
export PYTHONPATH="$root/scratch/apk-analysis-runtime/python:$root/tmp/conformance-cleanup/native-capture-a241793/conformance"
for order in join reverse; do
 "$python_bin" "$case_dir/dual-$order.py" > "$work/$order.log" 2>&1
 "$python_bin" "$root/tools/marker3-independent-scene/verify-native.py" "$work/$order.log" "$case_dir/drawing-paint-positive.json" "$([ "$order" = join ] && echo forward || echo reverse)"
 "$python_bin" - "$case_dir/drawing-paint-positive.rgba" "$root/tools/marker3-independent-scene/$([ "$order" = join ] && echo forward || echo reverse).rgba.zlib" <<'PY'
import hashlib,sys,zlib
from pathlib import Path
actual=Path(sys.argv[1]).read_bytes(); expected=zlib.decompress(Path(sys.argv[2]).read_bytes())
assert actual==expected, (hashlib.sha256(actual).hexdigest(),hashlib.sha256(expected).hexdigest())
print(hashlib.sha256(actual).hexdigest())
PY
done
