#!/usr/bin/env python3
"""Compare Crayon2 host readbacks with two caller MXCSR rounding modes."""

import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import zlib

if sys.flags.optimize:
    raise SystemExit("optimized Python disables required capture assertions")

ROOT = Path(os.environ.get("SDOCX_ROOT", Path(__file__).resolve().parents[2])).resolve()
SOURCE = (
    ROOT / "tmp/vector-parity-plan/crayon2-v26-upload-owner-join/capture_one_record.py"
)
PIN = "e2c9552a83e139694076e3f15bd69dee2c6fcb47e5d7507b0f4ed34aff5b0701"
SENTINEL = "1a9775e10bfa36c653f7e57cad5eba47decace7e34fce074f2d9286f0dd27d53"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def replace_once(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


assert sha(SOURCE.read_bytes()) == PIN, "V26 capture input differs"
with tempfile.TemporaryDirectory(prefix="crayon-round-") as tmp:
    tmp = Path(tmp)
    c = tmp / "mxcsr.c"
    c.write_text(
        "#include <xmmintrin.h>\nunsigned read_mxcsr(void){return _mm_getcsr();}\nvoid set_round(unsigned mode){_mm_setcsr((_mm_getcsr() & ~0x6000u) | mode);}\n"
    )
    so = tmp / "mxcsr.so"
    subprocess.run(["cc", "-shared", "-fPIC", "-O2", str(c), "-o", str(so)], check=True)
    s = SOURCE.read_text()
    s = replace_once(
        s,
        "PROJECT_ROOT = Path(__file__).resolve().parents[3]",
        f"PROJECT_ROOT = Path({str(ROOT)!r})",
    )
    s = replace_once(
        s,
        "OUTPUT_ROOT = PROJECT_ROOT / 'tmp/vector-parity-plan/crayon2-v26-upload-owner-join/v26-run'",
        f"OUTPUT_ROOT = Path({str(tmp / 'out')!r})",
    )
    s = replace_once(s, "str(trace_path.relative_to(PROJECT_ROOT))", "str(trace_path)")
    old = (
        " p.gles.f['glDrawArraysInstanced'](*vals);draw_error=p.gles.f['glGetError']()"
    )
    new = (
        f" maps=Path('/proc/self/maps').read_text();dri=next(line.split()[-1] for line in maps.splitlines() if 'libgallium' in line);print('GALLIUM_SHA256',dri,sha256(Path(dri).read_bytes()),flush=True)\n mx=C.CDLL({str(so)!r});mx.set_round(int(__import__('os').environ['PROBE_ROUND'],0));print('MXCSR_BEFORE',hex(mx.read_mxcsr()),flush=True)\n"
        + old
        + "\n print('MXCSR_AFTER',hex(mx.read_mxcsr()),flush=True)"
    )
    s = replace_once(s, old, new)
    s, count = re.subn(r"SELF_PIN='[0-9a-f]{64}'", f"SELF_PIN='{SENTINEL}'", s, count=1)
    assert count == 1
    s = s.replace(f"SELF_PIN='{SENTINEL}'", f"SELF_PIN='{sha(s.encode())}'", 1)
    runner = tmp / "capture.py"
    runner.write_text(s)
    captures = []
    for mode in (0, 0x2000):
        env = os.environ | {
            "LP_NUM_THREADS": "0",
            "PYTHONPATH": str(ROOT / "scratch/apk-analysis-runtime/python"),
            "CRAYON2_ARGB": "0x80336699",
            "PROBE_ROUND": hex(mode),
        }
        result = subprocess.run(
            [
                str(ROOT / ".venv/bin/python"),
                str(runner),
                "--_capture-background",
                "transparent",
                "--_translation-x",
                "0",
                "--_translation-y",
                "0",
                "--_host-dither",
                "preserve",
            ],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
        )
        if result.returncode:
            raise RuntimeError(result.stderr[-4000:])
        dri = re.search(r"GALLIUM_SHA256 (\S+) ([0-9a-f]{64})", result.stdout)
        assert dri
        before = re.search(r"MXCSR_BEFORE (0x[0-9a-f]+)", result.stdout)
        after = re.search(r"MXCSR_AFTER (0x[0-9a-f]+)", result.stdout)
        assert before and after
        capture = json.loads(result.stdout[result.stdout.index("{") :])
        paint = capture["paint"]
        assert paint["operation_errors"] == []
        assert len(paint["draw_calls"]) == 1 and paint["draw_calls"][0]["values"] == [
            5,
            0,
            4,
            1,
        ]
        pixels = zlib.decompress(
            base64.b64decode(paint["readback"]["rgba8_zlib_base64"])
        )
        assert len(pixels) == 256 * 256 * 4
        captures.append(
            (
                int(before.group(1), 16),
                int(after.group(1), 16),
                pixels,
                paint,
                dri.groups(),
            )
        )
    (b0, a0, p0, c0, d0), (b1, a1, p1, c1, d1) = captures
    assert d0 == d1 and c0["operation_errors"] == c1["operation_errors"] == []
    assert (
        c0["execution_closure"]["native_dso_sha256"]
        == c1["execution_closure"]["native_dso_sha256"]
    )
    assert (
        c0["execution_closure"]["host_gl_sha256"]
        == c1["execution_closure"]["host_gl_sha256"]
    )
    assert b0 & 0x6000 == a0 & 0x6000 == 0
    assert b1 & 0x6000 == a1 & 0x6000 == 0x2000
    changes = [
        (i // 4 % 256, i // 1024, i % 4, x, y)
        for i, (x, y) in enumerate(zip(p0, p1))
        if x != y
    ]
    receipt = {
        "schema": "crayon2-mxcsr-inline-rounding-v1",
        "boundary": "One synthetic replayed native RTV1 record on host Mesa with LP_NUM_THREADS=0; mixed pipeline, not an isolated byte-conversion test or device evidence.",
        "source_sha256": PIN,
        "loaded_gallium_path": d0[0],
        "loaded_gallium_sha256": d0[1],
        "nearest": {
            "mxcsr_before": hex(b0),
            "mxcsr_after": hex(a0),
            "rgba_sha256": sha(p0),
        },
        "downward": {
            "mxcsr_before": hex(b1),
            "mxcsr_after": hex(a1),
            "rgba_sha256": sha(p1),
        },
        "changed_channels": len(changes),
        "changes_xy_channel_nearest_down": changes,
        "native_dso_sha256": c0["execution_closure"]["native_dso_sha256"],
        "host_gl_sha256": c0["execution_closure"]["host_gl_sha256"],
        "gl_operation_errors": c0["operation_errors"],
    }
    assert changes, "no discriminating readback difference"
    print(json.dumps(receipt, indent=2, sort_keys=True))
