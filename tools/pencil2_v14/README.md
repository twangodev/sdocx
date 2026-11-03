# Synthetic mixed RGB native capture

The capture uses the tracked compressed base source and pinned local native inputs. From the repository root, restore the base source to the path expected by the unchanged capture script, then run it:

```sh
mkdir -p tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38
gzip -dc tools/pencil2_v14/context-base-capture.py.gz > tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 tools/pencil2_v14/capture.py
```

The script checks the base source and native library pins. It also requires the pinned local native inputs and Python runtime used by the base capture. The generated receipt records synthetic host-Mesa results.
