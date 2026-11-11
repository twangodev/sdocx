"""Maintained edge, horizontal viewport and projection ULP capture entrypoint."""

import argparse
import hashlib
import json
from pathlib import Path
import sys
import tempfile

from control_adapter import SOURCE_RELATIVE, SOURCE_SHA256, prepare_program
from streams import load_control_streams

ROOT = Path(__file__).resolve().parents[2]
HISTORICAL_PROGRAM_SHA256 = {
    "edge": "fcabffbf2f72c7b4623ec2e8753d33be763abdbd68a581d3da06fd1d755e984f",
    "horizontal": "aa47212e42d7641485db12721fcc2dddb60991a3b124b40a253c2fcc4f7fdeaa",
    "projection": "860c98dd190b05a0bcfdea04b5ff9b9034565b8f7047a5f078cea0799f53f6d6",
}
HISTORICAL_RECEIPTS = {"edge": "edge-interior", "horizontal": "horizontal-viewport", "projection": "projection-ulp"}


def check_control(root, variant):
    receipt_path = root / f"conformance/pencil2-v14-{HISTORICAL_RECEIPTS[variant]}-native.json"
    receipt = json.loads(receipt_path.read_text())
    streams = load_control_streams(root)
    hashes = {name: hashlib.sha256(data).hexdigest() for name, data in streams.items()}
    if hashes != receipt["inputs"]:
        raise ValueError("Constructed streams differ from historical receipt")
    historical = root / receipt["capture"]["script"]
    if hashlib.sha256(historical.read_bytes()).hexdigest() != receipt["capture"]["script_sha256"]:
        raise ValueError("Historical producer script no longer matches receipt")
    program = prepare_program(root, variant)
    if hashlib.sha256(program.encode()).hexdigest() != HISTORICAL_PROGRAM_SHA256[variant]:
        raise ValueError("Generated program differs from statically verified historical program")
    compile(program, str(root / SOURCE_RELATIVE), "exec")
    print(f"{variant}: retained stream hashes, historical producer hash and exact program checks passed")


def capture_control(root, variant):
    if sys.flags.optimize:
        raise ValueError("native capture requires assertions; run without Python optimization")
    streams = load_control_streams(root)
    program = prepare_program(root, variant)
    namespace = {"__name__": "edge_interior_capture", "__file__": str(root / SOURCE_RELATIVE),
                 "EDGE_STREAMS": streams, "EDGE_RESULTS": {}, "CLIP_MODE": [0], "MATRIX_EVENTS": []}
    exec(compile(program, str(root / SOURCE_RELATIVE), "exec"), namespace)
    with tempfile.TemporaryDirectory(prefix="sdocx-pencil2-control-") as directory:
        baseline = namespace["capture"](Path(directory))
    results = namespace["EDGE_RESULTS"]
    page = json.loads((root / "conformance/pencil2-v14-page-context-native.json").read_text())
    if results["mixed:0"]["sha256"] != page["results"]["mixed_top_scale2"]["sha256"]:
        raise ValueError("Unmodified control differs from retained page-context baseline")
    expected_count = len(streams) * (3 if variant == "edge" else 2)
    if len(results) != expected_count:
        raise ValueError(f"Expected {expected_count} control results, got {len(results)}")
    helpers = ("run_control.py", "streams.py", "control_adapter.py")
    return {
        "schema": "sdocx-pencil2-v14-maintained-control-v1", "variant": variant,
        "scope": "Synthetic 84-record host Mesa control; not device or saved-note evidence. "
                 "Edge translations change device-coordinate grain. Horizontal viewport and "
                 "projection interventions may alter interpolation and coverage.",
        "frame": [192, 192], "coordinates": "bottom-origin premultiplied RGBA8; transparent clear",
        "capture": {"base_source_sha256": SOURCE_SHA256,
                    "generated_program_sha256": hashlib.sha256(program.encode()).hexdigest(),
                    "maintained_source_sha256": {name: hashlib.sha256((Path(__file__).parent / name).read_bytes()).hexdigest()
                                                 for name in helpers},
                    "baseline_sha256": baseline["mesa"]["readback_sha256"], "dso_pins": namespace["DSO_PINS"]},
        "inputs": {name: hashlib.sha256(data).hexdigest() for name, data in streams.items()},
        "results": results,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("variant", choices=("edge", "horizontal", "projection"))
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="check retained evidence without native execution")
    mode.add_argument("--output", type=Path, help="new receipt path; must not exist")
    args = parser.parse_args()
    if args.check:
        check_control(ROOT, args.variant)
        return
    if args.output.exists():
        parser.error("--output must not already exist")
    if not args.output.parent.is_dir():
        parser.error("--output parent directory must exist")
    receipt = capture_control(ROOT, args.variant)
    with args.output.open("x") as output:
        output.write(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(args.output)


if __name__ == "__main__":
    main()
