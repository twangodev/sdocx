#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
revision=ef6a2e2ee2fff6e54af1eed0648cd23d5393e68b
python="$root/.venv/bin/python"
readers=${PENCIL_MESH_READERS:-/tmp/sdocx-marker3-v133-readers}
work=$(mktemp -d /tmp/sdocx-pencil-v309-replay.XXXXXXXX)
trap 'rm -rf "$work"' EXIT
cd "$root"
python3 tools/pencil2-mesh-v309/verify_inputs.py --native-local
mkdir -p "$work/repo" "$work/blue" "$work/white" "$work/geometry"
SDOCX_PENCIL2_FRESH_FRAME_DIR="$work/blue" python3 tools/pencil2-mesh-v309/capture.py > "$work/blue/capture.json"
SDOCX_PENCIL2_CLEAR_RGBA=255,255,255,255 SDOCX_PENCIL2_FRESH_FRAME_DIR="$work/white" python3 tools/pencil2-mesh-v309/capture.py > "$work/white/capture.json"
git archive "$revision" | tar -x -C "$work/repo"
cp -a tools/pencil2-mesh-v309 "$work/repo/tools/pencil2-mesh-v309"
cd "$work/repo"
python3 tools/pencil2-mesh-v309/verify_inputs.py --workspace
PENCIL_MESH_GEOMETRY_OUT="$work/geometry" bash tools/pencil2-mesh-v309/geometry.sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 cargo build --offline --locked --manifest-path tools/pencil2-mesh-v309/Cargo.toml --features export-pdf
PENCIL_MESH_GEOMETRY_OUT="$work/geometry" PYTHONPATH="$readers" "$python" tools/pencil2-mesh-v309/synthetic.py | tee "$work/synthetic.log"
for backdrop in blue white; do
 PENCIL_MESH_CAPTURE_DIR="$work/$backdrop" PENCIL_MESH_BACKDROP="$backdrop" PENCIL_MESH_GEOMETRY_OUT="$work/geometry" PENCIL_MESH_SVG="$work/$backdrop.svg" PYTHONPATH="$readers" "$python" tools/pencil2-mesh-v309/scene.py | tee "$work/$backdrop-mesh.log"
 PENCIL_MESH_CAPTURE_DIR="$work/$backdrop" PENCIL_MESH_BACKDROP="$backdrop" PYTHONPATH="$readers" "$python" tools/pencil2-mesh-v309/public_compare.py | tee "$work/$backdrop-generic.log"
done
python3 tools/pencil2-mesh-v309/check_metrics.py "$work"
