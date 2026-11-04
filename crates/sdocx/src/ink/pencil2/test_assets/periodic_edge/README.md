# Periodic edge native control

`native.json` contains the synthetic host Mesa capture; its
SHA-256 is `345c5d0804d640110d3480e823ed4d7fdfadcf2179a5600e9f26f88b655d2f79`. The receipt pins the capture
script, input streams, and baseline, and records DSO pin constants. The original
capture bypassed DSO preflight: the hashes of the libraries actually loaded
were not verified, so their identity is unconfirmed. This is not saved-note or
device evidence.

The unchanged capture script is `tools/pencil2_v14/periodic_edge_capture.py`.
Restore its pinned base source (SHA-256
`34d5874cb86e27b619c01e9902b0aacf743619be560d8a4ca5ee65789ca8193d`)
from the tracked `tools/pencil2_v14/context-base-capture.py.gz`:

```sh
mkdir -p tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38
gzip -dc tools/pencil2_v14/context-base-capture.py.gz > tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 tools/pencil2_v14/periodic_edge_capture.py
cp conformance/pencil2-v14-periodic-edge-native.json crates/sdocx/src/ink/pencil2/test_assets/periodic_edge/native.json
```

Before recapture, manually compute the SHA-256 of each DSO to be loaded and
compare it with the corresponding `DSO_PINS` entry in the restored base source;
proceed only when all match. Recapture also requires the historical inputs
listed by the base source and the local emulator runtime.

The capture writes `conformance/pencil2-v14-periodic-edge-native.json` as a
temporary output; the asset is the retained copy.
