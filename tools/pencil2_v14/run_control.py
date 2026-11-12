"""Maintained edge, viewport, projection ULP and translation capture entrypoint."""

import argparse
import hashlib
import json
from pathlib import Path
import sys
import tempfile

from control_adapter import (
    SOURCE_RELATIVE, SOURCE_SHA256, historical_translation_program, prepare_program,
)
from streams import load_control_streams
from translation_cases import load_translation_cases

ROOT = Path(__file__).resolve().parents[2]
HISTORICAL_PROGRAM_SHA256 = {
    "edge": "fcabffbf2f72c7b4623ec2e8753d33be763abdbd68a581d3da06fd1d755e984f",
    "horizontal": "aa47212e42d7641485db12721fcc2dddb60991a3b124b40a253c2fcc4f7fdeaa",
    "projection": "860c98dd190b05a0bcfdea04b5ff9b9034565b8f7047a5f078cea0799f53f6d6",
    "translation": "ae5a97ba17a2d37c59ace2e540f3430d54261cda0d1d9447da379bf1b9620968",
}
HISTORICAL_RECEIPTS = {"edge": "edge-interior", "horizontal": "horizontal-viewport", "projection": "projection-ulp"}


TRANSLATION_INPUTS_SHA256 = "65bdb69a8140a53e1b56e67c826ed49ff5be1983028f1e4b57b8bc6b50bbd58e"


def translation_setup(root):
    cases = load_translation_cases(root)
    hashes = [(case.receipt_key, hashlib.sha256(case.stream).hexdigest()) for case in cases]
    ordered_hash = hashlib.sha256(json.dumps(hashes, separators=(",", ":")).encode()).hexdigest()
    if len(cases) != 120 or ordered_hash != TRANSLATION_INPUTS_SHA256:
        raise ValueError("Translation cases differ from historical input bytes, keys or order")
    for case in cases:
        x, down, stream_name = case.receipt_key.split("_", 2)
        if (int(x), int(down), stream_name) != (case.x_pixels, case.down_pixels, case.stream_name):
            raise ValueError("Named coordinates differ from historical key interpretation")
    program = prepare_program(root, "translation")
    historical = historical_translation_program(program)
    if hashlib.sha256(historical.encode()).hexdigest() != HISTORICAL_PROGRAM_SHA256["translation"]:
        raise ValueError("Translation program differs beyond the reviewed named-case statements")
    compile(program, str(root / SOURCE_RELATIVE), "exec")
    compile(historical, str(root / SOURCE_RELATIVE), "exec")
    return cases, program


def check_control(root, variant):
    if variant == "translation":
        translation_setup(root)
        print("translation: 120 ordered inputs, named coordinates and generated-program equivalence passed; 240 expected results")
        return
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
    cases = None
    if variant == "translation":
        cases, program = translation_setup(root)
        streams = {case.receipt_key: case.stream for case in cases}
    else:
        streams = load_control_streams(root)
        program = prepare_program(root, variant)
    namespace = {"__name__": "edge_interior_capture", "__file__": str(root / SOURCE_RELATIVE),
                 "EDGE_STREAMS": streams, "EDGE_RESULTS": {}, "CLIP_MODE": [0], "MATRIX_EVENTS": []}
    if cases is not None:
        namespace["TRANSLATION_CASES"] = cases
    exec(compile(program, str(root / SOURCE_RELATIVE), "exec"), namespace)
    with tempfile.TemporaryDirectory(prefix="sdocx-pencil2-control-") as directory:
        baseline = namespace["capture"](Path(directory))
    results = namespace["EDGE_RESULTS"]
    page = json.loads((root / "conformance/pencil2-v14-page-context-native.json").read_text())
    baseline_key = "0_0_mixed:0" if variant == "translation" else "mixed:0"
    if results[baseline_key]["sha256"] != page["results"]["mixed_top_scale2"]["sha256"]:
        raise ValueError("Unmodified control differs from retained page-context baseline")
    expected_count = len(streams) * (3 if variant == "edge" else 2)
    if len(results) != expected_count:
        raise ValueError(f"Expected {expected_count} control results, got {len(results)}")
    helpers = ("run_control.py", "streams.py", "control_adapter.py")
    if variant == "translation":
        helpers += ("translation_cases.py",)
    return {
        "schema": "sdocx-pencil2-v14-maintained-control-v1", "variant": variant,
        "scope": "Synthetic 84-record host Mesa control; not device or saved-note evidence. "
                 "Edge translations change device-coordinate grain. Horizontal viewport and "
                 "projection interventions may alter interpolation and coverage."
                 + (" Expanded vertical viewport translations preserve nominal framebuffer coordinates "
                    "but may change interpolation and grain." if variant == "translation" else ""),
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
    parser.add_argument("variant", choices=("edge", "horizontal", "projection", "translation"))
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
