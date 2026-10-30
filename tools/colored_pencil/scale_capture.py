"""Observe GLV4's display-to-background-scale producer and queued native setter."""

import argparse
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

import context_capture as acquire

ROOT = Path(__file__).resolve().parents[2]
ACQUIRE = ROOT / "tools/colored_pencil/context_capture.py"
ACQUIRE_SHA = "8798fd6426c76c7660484eccdb95f91a24ed803a09361f606b5466765bf5db47"
GEOMETRY = ROOT / "conformance/colored-pencil-v4-source-context-native.json"
IDS = [
    "size_8",
    "screen_1080x1920",
    "screen_800x600",
    "screen_640x480",
    "screen_2560x1440",
    "screen_3840x2160",
    "screen_1x16384",
    "screen_16384x16384",
]

OBSERVER = """def observe_background_scale(u,address,size,data):
 if address not in (0x502a6b0,0x502a6bc,0x502a6cc,0x502c974,0x502c978):return
 row={'phase':phase,'pc_rva':hex(address-0x5000000),'queue_depth':len(p._task_returns)}
 if address==0x502a6b0:
  pen_data=u.reg_read(R.UC_ARM64_REG_X8)
  row.update({'kind':'producer_pen_data','dimensions':list(p.get(pen_data+32,'2i'))})
 elif address==0x502a6bc:
  row.update({'kind':'producer_minimum','minimum':u.reg_read(R.UC_ARM64_REG_W8)})
 elif address==0x502a6cc:
  row.update({'kind':'producer_result','f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex()})
 else:
  receiver=u.reg_read(R.UC_ARM64_REG_X0)
  row.update({'kind':'setter_entry' if address==0x502c974 else 'setter_stored','receiver':hex(receiver),'s0_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'stored_f32_bits':bytes(u.mem_read(receiver+264,4)).hex(),'caller_lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})
 SCALE_EVENTS.append(row)
p.u.hook_add(UC_HOOK_CODE,observe_background_scale)
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def capture(case):
    source = ACQUIRE.read_text()
    assert sha(source.encode()) == ACQUIRE_SHA
    tree = ast.parse(source)
    function = next(
        n
        for n in tree.body
        if isinstance(n, ast.FunctionDef) and n.name == "run_geometry"
    )
    body = ast.get_source_segment(source, function)
    target = "    namespace = {\n"
    assert body.count(target) == 1
    body = body.replace(
        target,
        "    source = source.replace('obj=p.alloc(0x100); impl=p.alloc(0x400)', SCALE_OBSERVER + 'obj=p.alloc(0x100); impl=p.alloc(0x400)')\n"
        + target,
    )
    target = '        "__name__": "colored_pencil_context_source",\n'
    assert body.count(target) == 1
    body = body.replace(target, target + '        "SCALE_EVENTS": SCALE_EVENTS,\n')
    scope = dict(vars(acquire), SCALE_OBSERVER=OBSERVER, SCALE_EVENTS=[])
    exec(compile(body, str(ACQUIRE), "exec"), scope)
    namespace, native = scope["run_geometry"](case)
    geometry = json.loads(GEOMETRY.read_bytes())
    original = next(row for row in geometry["cases"] if row["id"] == case["id"])
    assert acquire.compact(case, native) == original
    rt = namespace["selected_rt"]
    probe = namespace["p"]
    stored = bytes(probe.u.mem_read(rt + 264, 4)).hex()
    events = scope["SCALE_EVENTS"]
    for phase in ("draw_object_stroke", "saved_redraw"):
        rows = [row for row in events if row["phase"] == phase]
        assert [row["kind"] for row in rows] == [
            "producer_pen_data",
            "producer_minimum",
            "producer_result",
            "setter_entry",
            "setter_stored",
        ], rows
        assert rows[0]["dimensions"] == case["screen"]
        assert (
            rows[2]["f32_bits"]
            == rows[3]["s0_f32_bits"]
            == rows[4]["stored_f32_bits"]
            == stored
        )
        assert rows[3]["receiver"] == rows[4]["receiver"] == hex(rt)
        assert rows[3]["queue_depth"] > 0
    assert len(events) == 10
    return {
        "id": case["id"],
        "display": case["screen"],
        "geometry_stream_sha256": original["stream_sha256"],
        "background_scale_f32_bits": stored,
        "events": events,
        "drawing_and_selected_redraw_geometry_unchanged": True,
    }, native["libraries"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--child")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.child:
        case = next(row for row in acquire.controls() if row["id"] == args.child)
        result, libraries = capture(case)
        args.output.write_text(
            json.dumps({"case": result, "libraries": libraries}, separators=(",", ":"))
        )
        return
    receipt = {
        "schema": "sdocx-colored-pencil-v4-background-scale-producer-native-v1",
        "capture": {
            "script": "tools/colored_pencil/scale_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "source_acquisition_sha256": ACQUIRE_SHA,
            "geometry_receipt_sha256": sha(GEOMETRY.read_bytes()),
            "source_controller_sha256": acquire.SOURCE_SHA,
            "canvas_helper_sha256": acquire.HELPER_SHA,
        },
        "scope": "Read-only native instruction hooks observe GLV4 PenData display dimensions, selected integer minimum, F32 division result and queued RTV1 SetBackgroundScale input/stored field. Native Drawing and direct selected Redraw execute under the existing explicit host display/identity/queue boundary. Every geometry receipt row must remain unchanged. No drawable-field, sampler, native instruction or scheduler patches are introduced. Not native SDK deserialization, actual device display selection, depth-map selection or renderer pixels.",
        "cases": [],
    }
    with tempfile.TemporaryDirectory(prefix="sdocx-colored-pencil-scale-") as directory:
        output = Path(directory) / "case.json"
        for identifier in IDS:
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--child",
                    identifier,
                    "--output",
                    str(output),
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=120,
            )
            if result.returncode:
                print(identifier, result.stderr[-6000:])
                raise SystemExit(result.returncode)
            native = json.loads(output.read_bytes())
            receipt["cases"].append(native["case"])
            receipt.setdefault("libraries", native["libraries"])
            print(identifier, native["case"]["background_scale_f32_bits"], flush=True)
    output = ROOT / "conformance/colored-pencil-v4-background-scale-native.json"
    temporary = output.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    temporary.replace(output)
    print(output, output.stat().st_size)


if __name__ == "__main__":
    main()
