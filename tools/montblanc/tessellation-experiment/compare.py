import os
import sys
import time
import json
import subprocess
import tempfile
import base64
import gzip
import zlib
import hashlib
import tarfile
from pathlib import Path
from PIL import Image
import pymupdf
import PIL

repo = Path(__file__).resolve().parents[3]
revision = "353d70034cc20706d81542d2e38bd5109a05321a"
tmp = tempfile.TemporaryDirectory(prefix="montblanc-tessellation-")
root = Path(tmp.name)
archive = root / "source.tar"
subprocess.run(
    ["git", "archive", "--format=tar", "--output", str(archive), revision],
    cwd=repo,
    check=True,
)
with tarfile.open(archive) as source_archive:
    source_archive.extractall(root, filter="data")
archive.unlink()
assert subprocess.check_output(["resvg", "--version"], text=True).strip() == "0.45.1"
assert pymupdf.VersionBind == "1.28.2" and PIL.__version__ == "12.1.1"
assert (
    hashlib.sha256(
        (
            root / "crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx"
        ).read_bytes()
    ).hexdigest()
    == "ec4476cba55a9e92e4d5c54eb5789e0842a223e120825009fffd004a952d7de5"
)
sys.path.insert(0, str(root / "tools/montblanc"))
from public_fallback_compare import composite_native_frame, metrics  # noqa: E402

source = root / "crates/sdocx/src/render/montblanc_candidate.rs"
original = source.read_text()
render = root / "crates/sdocx/src/render.rs"
renderoriginal = render.read_text()
result = {}
env = os.environ | {
    "CARGO_TARGET_DIR": "/tmp/sdocx-agent-target",
    "CARGO_INCREMENTAL": "0",
    "CARGO_PROFILE_DEV_DEBUG": "0",
    "CARGO_PROFILE_TEST_DEBUG": "0",
}
refs1 = {}
for case in ["baseline", "pressure", "size", "far_endpoint", "four", "five"]:
    p = root / (
        f"tools/montblanc/extended-paint/{case}.json.gz"
        if case in ("four", "five")
        else f"tools/montblanc/captures/{case}.json.gz"
    )
    capture = gzip.decompress(p.read_bytes())
    receipt = json.loads(
        (
            root
            / (
                "conformance/montblanc-extended-paint-native.json"
                if case in ("four", "five")
                else "conformance/montblanc-public-fallback-host-comparison.json"
            )
        ).read_text()
    )
    assert (
        hashlib.sha256(capture).hexdigest() == receipt["cases"][case]["capture_sha256"]
    )
    data = json.loads(capture)
    raw = zlib.decompress(base64.b64decode(data["post_readback"]["rgba8_zlib_base64"]))
    assert hashlib.sha256(raw).hexdigest() == data["post_readback"]["sha256"]
    b = (
        Image.frombytes("RGBA", (256, 256), raw)
        .transpose(Image.Transpose.FLIP_TOP_BOTTOM)
        .tobytes()
    )
    refs1[case] = bytes(
        min(255, b[i + j] + round(252 * (255 - b[i + 3]) / 255))
        for i in range(0, len(b), 4)
        for j in range(3)
    )
refs2 = {}
native = json.loads(
    (root / "conformance/montblanc-native-scale-2x-variants.json").read_text()
)
for case, info in native["cases"].items():
    raw = (root / f"conformance/montblanc-scale-2x-{case}.rgba").read_bytes()
    assert hashlib.sha256(raw).hexdigest() == info["crop_sha256"]
    left, top, right, bottom = info["bounds_half_open"]
    frame = Image.new("RGBA", (256, 256))
    frame.paste(Image.frombytes("RGBA", (right - left, bottom - top), raw), (left, top))
    refs2[case] = composite_native_frame(frame.tobytes(), (256, 256)).tobytes()
raw = (root / "conformance/montblanc-scale-2x.rgba").read_bytes()
assert (
    hashlib.sha256(raw).hexdigest()
    == json.loads(
        (root / "conformance/montblanc-scene-scale-2x-comparison.json").read_text()
    )["native_crop_sha256"]
)
frame = Image.new("RGBA", (256, 256))
frame.paste(Image.frombytes("RGBA", (47, 14), raw), (125, 124))
refs2["baseline"] = composite_native_frame(frame.tobytes(), (256, 256)).tobytes()
extended = json.loads(
    (root / "conformance/montblanc-extended-2x-comparison.json").read_text()
)
for case, info in extended.items():
    raw = (root / f"conformance/montblanc-{case}-2x.rgba").read_bytes()
    assert hashlib.sha256(raw).hexdigest() == info["crop_sha256"]
    left, top, right, bottom = info["bounds"]
    frame = Image.new("RGBA", (256, 256))
    frame.paste(Image.frombytes("RGBA", (right - left, bottom - top), raw), (left, top))
    assert hashlib.sha256(frame.tobytes()).hexdigest() == info["frame_sha256"]
    refs2[case] = composite_native_frame(frame.tobytes(), (256, 256)).tobytes()
for n in (8, 12, 16):
    assert (
        original.count("checked_mul(64)") == 1
        and original.count("pieces(buffers, 8, scale)") == 1
    )
    assert (
        original.count("32_768") == 1
        and renderoriginal.count("montblanc_remaining_patches: Cell::new(32_768)") == 1
    )
    source.write_text(
        original.replace("checked_mul(64)", f"checked_mul({n * n})")
        .replace("pieces(buffers, 8, scale)", f"pieces(buffers, {n}, scale)")
        .replace("32_768", "1_000_000")
    )
    render.write_text(
        renderoriginal.replace(
            "montblanc_remaining_patches: Cell::new(32_768)",
            "montblanc_remaining_patches: Cell::new(1_000_000)",
        )
    )
    result[str(n)] = {}
    for scale, refs in [(1, refs1), (2, refs2)]:
        with tempfile.TemporaryDirectory() as d:
            out = Path(d)
            manifest = (
                root
                / f"tools/montblanc/public-optin{'-2x' if scale == 2 else ''}/Cargo.toml"
            )
            t = time.monotonic()
            subprocess.run(
                [
                    "cargo",
                    "run",
                    "--offline",
                    "--quiet",
                    "--manifest-path",
                    str(manifest),
                    "--",
                    str(out),
                ],
                cwd=root,
                env=env,
                check=True,
            )
            elapsed = time.monotonic() - t
            rows = {}
            runs = []
            executable = Path("/tmp/sdocx-agent-target/debug") / (
                "montblanc-public-optin-2x-probe"
                if scale == 2
                else "montblanc-public-optin-probe"
            )
            for _ in range(3):
                start = time.monotonic()
                subprocess.run(
                    [str(executable), str(out)], cwd=root, env=env, check=True
                )
                runs.append(round(time.monotonic() - start, 3))
            for case, ref in refs.items():
                assert len(ref) == 256 * 256 * 3
                mode = "aware" if scale == 2 else "candidate"
                svg = out / f"{case}-{mode}.svg"
                pdf = out / f"{case}-{mode}.pdf"
                png = out / "raster.png"
                subprocess.run(
                    ["resvg", "--width", "256", str(svg), str(png)],
                    check=True,
                    stdout=subprocess.DEVNULL,
                )
                image = Image.open(png)
                assert image.size == (256, 256)
                actual = image.convert("RGB").tobytes()
                assert len(actual) == 256 * 256 * 3
                with pymupdf.open(pdf) as doc:
                    pix = doc[0].get_pixmap(
                        matrix=pymupdf.Matrix(
                            8 / 3 if scale == 2 else 4 / 3,
                            8 / 3 if scale == 2 else 4 / 3,
                        ),
                        alpha=False,
                    )
                    assert (pix.width, pix.height, pix.n) == (256, 256, 3)
                    pdfbytes = pix.samples
                    assert len(pdfbytes) == 256 * 256 * 3
                rows[case] = {
                    "svg": metrics(ref, actual),
                    "pdf": metrics(ref, pdfbytes),
                    "svg_bytes": svg.stat().st_size,
                    "pdf_bytes": pdf.stat().st_size,
                }
            result[str(n)][str(scale)] = {
                "run_seconds_including_cargo": round(elapsed, 3),
                "warm_runs_seconds": runs,
                "cases": rows,
            }
            print(n, scale, round(elapsed, 2), flush=True)
metadata = {
    "revision": revision,
    "versions": {"resvg": "0.45.1", "pillow": "12.1.1", "pymupdf": "1.28.2"},
    "metrics": [
        "native_support",
        "export_support",
        "intersection",
        "union",
        "rgb_l1_on_union",
    ],
    "support": "any channel differs from paper 252 by more than 2",
    "timing": "seconds for full six-case three-mode SVG and PDF probe, including writes",
}
lines = ["{", '  "metadata": ' + json.dumps(metadata) + ",", '  "results": {']
for ni, (n, scales) in enumerate(result.items()):
    lines.append("    " + json.dumps(n) + ": {")
    for si, (scale, data) in enumerate(scales.items()):
        lines.append("      " + json.dumps(scale) + ": {")
        lines.append(
            '        "run_seconds_including_cargo": '
            + json.dumps(data["run_seconds_including_cargo"])
            + ","
        )
        lines.append(
            '        "warm_runs_seconds": '
            + json.dumps(data["warm_runs_seconds"])
            + ","
        )
        lines.append('        "cases": {')
        for ci, (case, row) in enumerate(data["cases"].items()):
            lines.append(
                "          "
                + json.dumps(case)
                + ": "
                + json.dumps(row)
                + ("," if ci + 1 < len(data["cases"]) else "")
            )
        lines.append("        }")
        lines.append("      }" + ("," if si + 1 < len(scales) else ""))
    lines.append("    }" + ("," if ni + 1 < len(result) else ""))
lines.extend(["  }", "}"])
Path(sys.argv[1]).write_text("\n".join(lines) + "\n")
