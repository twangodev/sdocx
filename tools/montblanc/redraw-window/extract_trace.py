#!/usr/bin/env python3
"""Extract compact native entry states and streams from trace.py stderr logs."""
import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[3]
directory = Path(sys.argv[1])
names = ("9", "10-distinct", "11", "12", "13", "duplicate10", "duplicate12", "pressure-duplicate", "short-move")
frozen = json.loads((Path(__file__).parent / "streams.json").read_text())["cases"]
cases = {}
for name in names:
    lines = (directory / f"mb-window-{name}.log").read_text().splitlines()
    stream = json.loads(next(line[15:] for line in lines if line.startswith("CAPTURE_STREAM=")))
    rows = json.loads(next(line[6:] for line in lines if line.startswith("STATE=")))
    end = next(row for row in rows if "::endPen(" in row["symbol"])
    draw = [row for row in rows if "::drawLine(" in row["symbol"]]
    if name in frozen:
        assert stream == frozen[name]["stream"], name
        source = frozen[name]["input"]
    else:
        source = json.loads((directory / f"mb-window-{name}-input.json").read_text())
    cases[name] = {"input": source, "stream": stream, "end_state": {k: end[k] for k in ("entry_pc", "lr", "x0_bytes", "motion_event_bytes")},
        "draw_calls": [{"lr": row["lr"], "s0_s5_f32_bits": row["s0_s5_f32_bits"], "receiver_bytes_80_144": row["drawable_state_bytes"][160:288]} for row in draw]}
receipt = {"trace_script_sha256": hashlib.sha256((Path(__file__).parent / "trace.py").read_bytes()).hexdigest(), "capture_script_sha256": hashlib.sha256((root / "tools/montblanc/narrow-probe/capture.py").read_bytes()).hexdigest(), "cases": cases}
print(json.dumps(receipt, indent=2))
