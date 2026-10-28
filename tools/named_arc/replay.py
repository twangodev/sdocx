import ast
import json
import re
import pathlib
import subprocess
import sys
import os
import tempfile

root = pathlib.Path(__file__).resolve().parent
assert len(sys.argv) == 5, "usage: replay.py PROBE MODEL BASE SKIA"
probe, model, base, skia = map(lambda p: str(pathlib.Path(p).resolve()), sys.argv[1:])
fixture = root.parent.parent / "conformance" / "named-arc-native.json"
temporary = tempfile.TemporaryDirectory()
output = pathlib.Path(temporary.name)
for fill in (0, 85, 165, 255):
    result = subprocess.run(
        [probe, model, "--named-arc", base, skia],
        env={**os.environ, "ARC_FILL": str(fill)},
        capture_output=True,
        text=True,
        check=True,
    )
    (output / f"run-{fill}.log").write_text(result.stdout + result.stderr)


def active(seg):
    v = seg[0]
    return [seg[i] for i in {1: [0, 1, 2], 2: [0, 1, 2], 3: [0, 1, 2, 5, 6], 6: [0]}[v]]


def parse(fill):
    log = (output / f"run-{fill}.log").read_text()
    assert (
        "panicked" not in log
        and "control 0 result 1" in log
        and "control 1 result 1" in log
    )
    out = {}
    for line in log.splitlines():
        if not (line.startswith(("path ", "fill ", "moved path ", "moved fill "))):
            continue
        key, record = (
            line.split(" segments ", 1) if " segments " in line else (None, None)
        )
        if record is None:
            m = re.match(r"(moved path|path) (\d+) (?:[0-9a-f]+ )?(\(.*\))", line)
            assert m, line
            key = f"{m[1]} {m[2]}"
            record = m[3]
        else:
            m = re.match(
                r"(moved fill|fill) (\d+) count 1 (?:item [0-9a-f]+ )?type 1", key
            )
            assert m, line
            key = f"{m[1]} {m[2]}"
        count, seg = ast.literal_eval(record)
        assert count == len(seg)
        out[key] = list(map(active, seg))
    assert len(out) == 8, out.keys()
    return out


runs = [parse(n) for n in [0, 85, 165, 255]]
assert all(x == runs[0] for x in runs)
r = {
    "target": "Samsung Notes 4.4.45.37 arm64",
    "rect": [10, 20, 210, 120],
    "control_changes": [[0, 20, 30], [1, 40, 100]],
    "fill_bytes": [0, 85, 165, 255],
    "coordinates": "active f32 bit patterns",
    "results": runs[0],
    "scope": "Native constructor, rectangle and control setters, path and fill records; no saved loader, Drawing canvas or device pixels. Linux libm substitutions and host memory/TLS/logging/mutex; unbound imports trap.",
}
expected = json.loads(fixture.read_text())
assert r == expected, "capture differs from committed receipt"
print(f"named Arc capture matches {fixture}")
