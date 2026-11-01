#!/usr/bin/env python3
"""Compare public exports. Requires pymupdf and pypdfium2; run from repository root."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import pymupdf
import pypdfium2

ROOT = Path(__file__).resolve().parents[3]
INPUTS = ["crates/sdocx/examples/montblanc_pdf_reuse_mixed_probe.rs", "crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx", "crates/sdocx/src/pdf/svg/group.rs", "crates/sdocx/src/pdf/svg/mod.rs", "crates/sdocx/src/render/montblanc_candidate.rs", "Cargo.lock", "crates/sdocx/Cargo.toml", "tools/montblanc/pdf-reuse-mixed-v305/compare.py"]
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def raster(path, reader, scale):
    if reader == "mupdf":
        doc = pymupdf.open(path)
        return bytes(doc[0].get_pixmap(matrix=pymupdf.Matrix(scale, scale), alpha=False).samples)
    doc = pypdfium2.PdfDocument(path)
    return bytes(doc[0].render(scale=scale).buffer)

def main():
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR="/tmp/sdocx-agent-target", CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0", CARGO_BUILD_JOBS="2")
    with tempfile.TemporaryDirectory(prefix="sdocx-mixed-reuse-") as temp:
        out = Path(temp)
        subprocess.run(["cargo", "run", "-p", "sdocx", "--features", "pdf,serde", "--example", "montblanc_pdf_reuse_mixed_probe", "--", str(out)], cwd=ROOT, env=env, check=True)
        results = {}
        for reader in ("mupdf", "pdfium"):
            for scale in (1, 2):
                for name in ("normal", "rotate", "reflect", "cancel"):
                    a = raster(out / f"{name}-marked.pdf", reader, scale)
                    b = raster(out / f"{name}-flat.pdf", reader, scale)
                    assert len(a) == len(b)
                    diffs = [abs(x-y) for x, y in zip(a, b)]
                    results[f"{reader}/{scale}x/{name}"] = {"changed_channels": sum(x != 0 for x in diffs), "max_delta": max(diffs), "sum_delta": sum(diffs)}
                a = raster(out / "direct.pdf", reader, scale)
                b = raster(out / "normal-flat.pdf", reader, scale)
                assert len(a) == len(b)
                diffs = [abs(x-y) for x, y in zip(a, b)]
                results[f"{reader}/{scale}x/direct"] = {"changed_channels": sum(x != 0 for x in diffs), "max_delta": max(diffs), "sum_delta": sum(diffs)}
        receipt = {"scope": "Synthetic parsed fixture with two offset Montblanc strokes and one interleaved ordinary object. Flat controls replace producer IDs after public SVG generation. Transforms wrap the public SVG page. Direct PDF is compared with normal flat SVG PDF. The reuse guard requires finite nonzero axis-aligned SVG entry, cumulative SVG, and current surface transforms; the Rust tests cover rotation, shear, caller transform, and registered native-ink descendant fallback. Native-ink hooks and caller surface transforms are not rasterized by this probe. No device claim.", "versions": {"mupdf": pymupdf.VersionBind, "pdfium": str(pypdfium2.PDFIUM_INFO)}, "inputs_sha256": {p: sha(ROOT / p) for p in INPUTS}, "exports_sha256": {p.name: sha(p) for p in sorted(out.iterdir())}, "results": results}
        if any(v["changed_channels"] for v in results.values()):
            raise SystemExit("render mismatch")
        receipt_path = ROOT / "tools/montblanc/pdf-reuse-mixed-v305/receipt.json"
        if sys.argv[1:] == ["--write-receipt"]:
            receipt_path.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
        elif not sys.argv[1:]:
            expected = json.loads(receipt_path.read_text())
            if receipt["versions"] != expected["versions"]:
                raise SystemExit(f"reader versions differ: {receipt['versions']} vs {expected['versions']}; receipt comparison requires matching readers")
            for key in ("inputs_sha256", "exports_sha256", "results"):
                if receipt[key] != expected[key]:
                    raise SystemExit(f"receipt mismatch: {key}")
        else:
            raise SystemExit("usage: compare.py [--write-receipt]")
        print(json.dumps(receipt, indent=2, sort_keys=True))
if __name__ == "__main__":
    main()
