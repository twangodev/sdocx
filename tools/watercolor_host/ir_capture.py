"""Retain compiler and redraw-state evidence for the pinned WaterColor host."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
FRAME = ROOT / "tools/watercolor_host/frame_capture.py"
FRAME_PIN = "d87055488b895e4ee6dce35ca06f615f69c04a2d2f1cf28d3b7ad9786618ffd8"
parser = argparse.ArgumentParser()
parser.add_argument("--consume-report", type=Path)
parser.add_argument("--consume-stderr", type=Path)
args = parser.parse_args()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def elf_bytes(path, address, length):
    raw = path.read_bytes()
    assert raw[:6] == b"\x7fELF\x02\x01"
    offset = struct.unpack_from("<Q", raw, 32)[0]
    size, count = struct.unpack_from("<HH", raw, 54)
    for i in range(count):
        kind, _, file_offset, virtual, _, file_size, _, _ = struct.unpack_from(
            "<IIQQQQQQ", raw, offset + i * size
        )
        if kind == 1 and virtual <= address and address + length <= virtual + file_size:
            start = file_offset + address - virtual
            return raw[start : start + length]
    raise ValueError("requested bytes not in file-backed ELF segment")


with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-ir-") as directory:
    directory = Path(directory)
    assert sha(FRAME.read_bytes()) == FRAME_PIN
    if args.consume_report:
        assert args.consume_stderr
        report = args.consume_report
        stderr = args.consume_stderr
    else:
        report = directory / "native.json"
        stderr = directory / "stderr"
        environment = dict(
            os.environ, GALLIVM_DEBUG="ir,asm", MESA_SHADER_CACHE_DISABLE="true"
        )
        with (directory / "stdout").open("wb") as stdout, stderr.open("wb") as err:
            subprocess.run(
                [
                    sys.executable,
                    str(FRAME),
                    "--child",
                    "--width",
                    "1920",
                    "--height",
                    "1080",
                    "--repeat",
                    "32",
                    "--output",
                    str(report),
                ],
                cwd=ROOT,
                env=environment,
                stdout=stdout,
                stderr=err,
                check=True,
                timeout=180,
            )
    native = json.loads(report.read_text())
    baseline = json.loads(
        (ROOT / "conformance/watercolor-redraw-frames-native.json").read_text()
    )["cases"]["1920x1080_1_32"]
    assert native["capture_status"] == "captured_without_host_gl_errors"
    assert native["readback"]["sha256"] == baseline["final_sha256"]
    assert (
        native["native_mask_pass_readback"]["pixels_sha256"] == baseline["mask_sha256"]
    )
    raw = stderr.read_bytes()
    lines = raw.decode().splitlines()

    def excerpt(start, stop):
        a = next(i for i, line in enumerate(lines) if start in line)
        b = next(i for i in range(a + 1, len(lines)) if stop in lines[i])
        return {"first_line": a + 1, "lines": lines[a:b]}

    sources = {}
    contracts = {}
    for name, address, length, claim in [
        (
            "PenCommon",
            0x4A4A4,
            8,
            "SetRedrawState(bool) stores w1 at this+25, then returns.",
        ),
        (
            "WaterColorBrush",
            0x65C18,
            40,
            "drawComposite loads this+25; nonzero skips the supplied target ClearColor virtual call with four zero float arguments.",
        ),
    ]:
        path = ROOT / f"scratch/apk-analysis-native/arm64-v8a/libSPen{name}.so"
        sources[name] = sha(path.read_bytes())
        contracts[name] = {
            "rva": hex(address),
            "bytes_hex": elf_bytes(path, address, length).hex(),
            "claim": claim,
        }
    receipt = {
        "schema": "sdocx-watercolor-host-pipeline-v1",
        "capture": {
            "script": "tools/watercolor_host/ir_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "frame_helper_sha256": FRAME_PIN,
            "stderr_sha256": sha(raw),
            "stderr_bytes": len(raw),
            "environment": {
                "GALLIVM_DEBUG": "ir,asm",
                "MESA_SHADER_CACHE_DISABLE": "true",
            },
        },
        "native": {
            "mask_sha256": baseline["mask_sha256"],
            "final_sha256": baseline["final_sha256"],
            "gles": native["gles"],
            "libraries": native["source"],
        },
        "redraw_contract": {
            "libraries_sha256": sources,
            "blocks": contracts,
            "scope": "Ordinary ELF byte reads and native public-API state controls, not REA decompilation. The older first-use-clear interpretation is replaced by the named redraw-state gate.",
        },
        "excerpts": {
            "mask_fragment_nir": excerpt(
                "shader: MESA_SHADER_FRAGMENT", "; ModuleID = 'fs"
            ),
            "mask_interpolation_llvm": excerpt(
                "  %109 = fadd <8 x float>",
                "  store <8 x i32> zeroinitializer, ptr %break_mask",
            ),
            "setup_coefficients": excerpt(
                "define void @setup_variant_0", "  %29 = getelementptr"
            ),
            "nearest_sampling_llvm": excerpt(
                "  %173 = fmul <8 x float>", "  %177 = sub <8 x i32>"
            ),
        },
        "scope": "Unchanged native shaders and fresh redraw=true native RTV1 Draw; explicit Mesa/glibc host boundary. Retains active plane setup, ordered FMA interpolation and floor/clamp nearest sampling. Not Android GPU or saved-page selection evidence.",
    }
    target = ROOT / "conformance/watercolor-host-pipeline.json"
    target.write_text(json.dumps(receipt, indent=2) + "\n")
    print(target, target.stat().st_size)
