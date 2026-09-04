import argparse
import hashlib
import html
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys

import numpy as np
from PIL import Image, ImageFilter
import PIL
import pymupdf


ROOT = Path(__file__).resolve().parent.parent


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def read_fixtures(manifest, corpus, selected):
    fixtures = []
    seen = set()
    for number, line in enumerate(manifest.read_text().splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 17 or not re.fullmatch(r"[A-Za-z0-9_-]+", fields[0]):
            raise ValueError(f"invalid manifest row {number}")
        fixture_id = fields[0]
        if fixture_id in seen:
            raise ValueError(f"duplicate fixture ID: {fixture_id}")
        seen.add(fixture_id)
        if selected and fixture_id not in selected:
            continue
        fixture = {"id": fixture_id, "visible_pages": int(fields[6])}
        if fixture["visible_pages"] < 1:
            raise ValueError(f"{fixture_id}: visible page count must be positive")
        for key, filename, digest in (
            ("sdocx", fields[1], fields[2]),
            ("reference_pdf", fields[3], fields[4]),
        ):
            path = (corpus / filename).resolve()
            if not path.is_relative_to(corpus.resolve()):
                raise ValueError(f"{fixture_id}: file is outside the corpus")
            if not re.fullmatch(r"[0-9a-f]{64}", digest) or sha256(path) != digest:
                raise ValueError(f"{fixture_id}: SHA-256 mismatch for {filename}")
            fixture[key] = path
            fixture[f"{key}_sha256"] = digest
        fixtures.append(fixture)
    if selected - seen:
        raise ValueError(f"unknown fixture IDs: {', '.join(sorted(selected - seen))}")
    if not fixtures:
        raise ValueError("no fixtures selected")
    return fixtures


def run(command, **kwargs):
    return subprocess.run(
        [str(part) for part in command],
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
        **kwargs,
    )


def font_inventory():
    """Record local font bytes, not just family names, when fontconfig exists."""
    if not shutil.which("fc-list"):
        return {"available": False, "reason": "fc-list is unavailable"}
    paths = sorted(set(run(["fc-list", "--format=%{file}\\n"]).stdout.splitlines()))
    fonts = [{"file": path, "sha256": sha256(Path(path))} for path in paths]
    # Ignore paths when comparing the inventory between machines.
    digests = "\n".join(sorted(font["sha256"] for font in fonts))
    return {
        "available": True,
        "sha256": hashlib.sha256(digests.encode()).hexdigest(),
        "fonts": fonts,
    }


def candidate_pages(directory, count):
    expected = (
        [directory / "sdk.png"]
        if count == 1
        else [directory / f"sdk_page{index}.png" for index in range(count)]
    )
    if set(directory.glob("sdk*.png")) != set(expected):
        raise ValueError(f"{directory.name}: SDK page count/names differ from manifest")
    return expected


def rasterize_reference(page, size):
    width, height = size
    rect = page.rect
    if width < 1 or height < 1 or rect.width <= 0 or rect.height <= 0:
        raise ValueError("invalid page dimensions")
    # Permit rounding by one pixel, not a different page aspect ratio.
    expected_height = width * rect.height / rect.width
    if abs(height - expected_height) > 1:
        raise ValueError(
            f"page aspect ratio mismatch: SDK {width}x{height}, PDF {rect.width}x{rect.height}"
        )
    raster = page.get_pixmap(
        matrix=pymupdf.Matrix(width / rect.width, height / rect.height),
        colorspace=pymupdf.csRGB,
        alpha=False,
    )
    if not (
        width <= raster.width <= width + 1 and height <= raster.height <= height + 1
    ):
        raise ValueError("unexpected PDF raster dimensions")
    image = Image.frombytes("RGB", (raster.width, raster.height), raster.samples)
    return image.crop((0, 0, width, height))


def rgb_on_white(image):
    rgba = image.convert("RGBA")
    return Image.alpha_composite(Image.new("RGBA", rgba.size, "white"), rgba).convert(
        "RGB"
    )


def dilate(mask, tolerance):
    if tolerance == 0:
        return mask
    return (
        np.asarray(
            Image.fromarray(mask.astype(np.uint8) * 255).filter(
                ImageFilter.MaxFilter(2 * tolerance + 1)
            )
        )
        != 0
    )


def measure(reference, candidate, channel_threshold=16, ink_threshold=32, tolerance=1):
    if reference.size != candidate.size:
        raise ValueError("pixel comparison requires matching dimensions")
    ref = np.asarray(rgb_on_white(reference), dtype=np.int16)
    sdk = np.asarray(rgb_on_white(candidate), dtype=np.int16)
    delta = np.abs(ref - sdk)
    ref_ink = np.min(ref, axis=2) < 255 - ink_threshold
    sdk_ink = np.min(sdk, axis=2) < 255 - ink_threshold
    ref_count, sdk_count = int(ref_ink.sum()), int(sdk_ink.sum())
    missing = int((ref_ink & ~dilate(sdk_ink, tolerance)).sum())
    extra = int((sdk_ink & ~dilate(ref_ink, tolerance)).sum())
    metrics = {
        "mean_absolute_rgb_error": float(delta.mean() / 255),
        "changed_pixel_fraction": float(
            np.any(delta > channel_threshold, axis=2).mean()
        ),
        "reference_ink_pixels": ref_count,
        "sdk_ink_pixels": sdk_count,
        "missing_ink_fraction": missing / ref_count if ref_count else 0.0,
        "extra_ink_fraction": extra / sdk_count if sdk_count else 0.0,
    }
    difference = Image.fromarray(np.clip(delta * 4, 0, 255).astype(np.uint8))
    return metrics, difference


def rasterize_candidate_pdf(path, count):
    pages = []
    with pymupdf.open(path) as pdf:
        if pdf.page_count != count:
            raise ValueError("SDK PDF page count differs from manifest")
        for index, page in enumerate(pdf):
            size = (round(page.rect.width * 96 / 72), round(page.rect.height * 96 / 72))
            image_path = path.with_name(
                "sdk.png" if count == 1 else f"sdk_page{index}.png"
            )
            rasterize_reference(page, size).save(image_path)
            text_path = path.with_name(f"sdk_page{index}.txt")
            text = page.get_text()
            text_path.write_text(text, encoding="utf-8")
            pages.append(
                {
                    "sdk_pdf_size_points": [page.rect.width, page.rect.height],
                    "sdk_text_characters": len(text),
                    "sdk_text": text_path.name,
                }
            )
    return pages


def compare_fixture(fixture, cli, output, options, font_files=(), output_format="png"):
    if output_format not in ("png", "pdf"):
        raise ValueError("SDK output format must be png or pdf")
    directory = output / fixture["id"]
    directory.mkdir()
    with pymupdf.open(fixture["reference_pdf"]) as pdf:
        if pdf.page_count != fixture["visible_pages"]:
            raise ValueError(f"{fixture['id']}: PDF page count differs from manifest")
        command = [
            cli,
            fixture["sdocx"],
            "--output",
            directory / f"sdk.{output_format}",
        ]
        for font in font_files:
            command.extend(["--font", font])
        conversion = run(command)
        (directory / "sdk.log").write_text(conversion.stdout + conversion.stderr)
        pdf_pages = (
            rasterize_candidate_pdf(directory / "sdk.pdf", pdf.page_count)
            if output_format == "pdf"
            else []
        )
        paths = candidate_pages(directory, pdf.page_count)
        pages = []
        for index, (reference_page, candidate_path) in enumerate(
            zip(pdf, paths, strict=True)
        ):
            with Image.open(candidate_path) as source:
                candidate = rgb_on_white(source)
            reference = rasterize_reference(reference_page, candidate.size)
            metrics, difference = measure(reference, candidate, **options)
            reference_path = directory / f"reference_page{index}.png"
            difference_path = directory / f"difference_page{index}.png"
            reference.save(reference_path)
            difference.save(difference_path)
            pages.append(
                {
                    "page": index + 1,
                    "size": list(candidate.size),
                    "pdf_size_points": [
                        reference_page.rect.width,
                        reference_page.rect.height,
                    ],
                    "metrics": metrics,
                    "reference": reference_path.relative_to(output).as_posix(),
                    "sdk": candidate_path.relative_to(output).as_posix(),
                    "difference": difference_path.relative_to(output).as_posix(),
                    "reference_png_sha256": sha256(reference_path),
                    "sdk_png_sha256": sha256(candidate_path),
                }
            )
            if pdf_pages:
                pages[-1].update(pdf_pages[index])
                pages[-1]["sdk_text"] = (
                    (directory / pdf_pages[index]["sdk_text"])
                    .relative_to(output)
                    .as_posix()
                )
    result = {
        "id": fixture["id"],
        "sdk_format": output_format,
        "sdocx_sha256": fixture["sdocx_sha256"],
        "reference_pdf_sha256": fixture["reference_pdf_sha256"],
        "pages": pages,
    }
    if output_format == "pdf":
        result["sdk_pdf"] = (directory / "sdk.pdf").relative_to(output).as_posix()
        result["sdk_pdf_sha256"] = sha256(directory / "sdk.pdf")
    return result


def write_html(report, destination):
    sections = []
    for fixture in report["fixtures"]:
        sections.append(f"<h2>{html.escape(fixture['id'])}</h2>")
        if "sdk_pdf" in fixture:
            pdf_link = html.escape(fixture["sdk_pdf"], quote=True)
            sections.append(f'<p><a href="{pdf_link}">Download SDK PDF</a></p>')
        for page in fixture["pages"]:
            metrics = page["metrics"]
            ref, sdk, diff = (
                html.escape(page[key], quote=True)
                for key in ("reference", "sdk", "difference")
            )
            sections.append(f"""
<details open><summary>Page {page["page"]} · changed {metrics["changed_pixel_fraction"]:.2%}
· missing ink {metrics["missing_ink_fraction"]:.2%} · extra ink {metrics["extra_ink_fraction"]:.2%}</summary>
<div class="pair"><figure><figcaption>Samsung PDF</figcaption><a href="{ref}"><img src="{ref}" alt="Samsung reference page"></a></figure>
<figure><figcaption>SDK</figcaption><a href="{sdk}"><img src="{sdk}" alt="SDK page"></a></figure></div>
<details><summary>Overlay and difference</summary>
<label>SDK overlay opacity <input type="range" min="0" max="100" value="50" aria-label="SDK overlay opacity"></label>
<div class="pair"><div class="overlay"><img src="{ref}" alt="Samsung reference"><img class="candidate" src="{sdk}" alt="SDK overlay"></div>
<figure><figcaption>Absolute RGB difference ×4 (black = identical)</figcaption><a href="{diff}"><img src="{diff}" alt="Amplified difference"></a></figure></div>
</details></details>""")
    destination.write_text(
        """<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>SDOCX visual comparison</title>
<style>body{font:16px system-ui,sans-serif;max-width:1500px;margin:32px auto;padding:0 24px;color:#252525;background:#f6f6f6}
h1{font-size:28px}p{max-width:85ch;line-height:1.5}.pair{display:grid;grid-template-columns:1fr 1fr;gap:16px;margin:12px 0 24px}
figure{margin:0}figcaption{margin-bottom:8px}img{display:block;width:100%;height:auto}summary{cursor:pointer;padding:12px 0}
.overlay{position:relative;align-self:start}.candidate{position:absolute;inset:0;opacity:.5}label{display:block;margin:12px 0}
@media(max-width:700px){.pair{grid-template-columns:1fr}}</style>
<h1>SDOCX visual comparison</h1><p>Samsung reference exports compared with SDK output in visible page order.
Page sizes are normalized without shifting or aligning content. Pixel scores include font and antialiasing differences;
they are measurements, not a compatibility verdict. Ink metrics assume a white or near-white canvas.
See <a href="report.json">report.json</a> for hashes, settings, tool versions, and font inventory.</p>
"""
        + "\n".join(sections)
        + """
<script>document.querySelectorAll('input[type=range]').forEach(input=>input.addEventListener('input',()=>{
input.closest('details').querySelector('.candidate').style.opacity=input.value/100;}));</script></html>
"""
    )


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Compare SDK PNG/PDF output with locked Samsung exports"
    )
    parser.add_argument(
        "--corpus-dir",
        type=Path,
        default=Path(os.environ.get("SDOCX_CORPUS_DIR", ROOT / "hf")),
    )
    parser.add_argument(
        "--manifest", type=Path, default=ROOT / "conformance/corpus.tsv"
    )
    parser.add_argument(
        "--fixture",
        action="append",
        default=[],
        help="fixture ID; repeat to select several",
    )
    parser.add_argument("--cli", type=Path, default=ROOT / "target/debug/sdocx-cli")
    parser.add_argument("--format", choices=("png", "pdf"), default="png")
    parser.add_argument(
        "--font",
        type=Path,
        action="append",
        default=[],
        help="font file passed to the CLI; repeat as needed",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="new directory; existing results are never reused",
    )
    parser.add_argument("--channel-threshold", type=int, default=16)
    parser.add_argument("--ink-threshold", type=int, default=32)
    parser.add_argument(
        "--tolerance",
        type=int,
        choices=range(0, 6),
        default=1,
        help="ink matching radius in pixels",
    )
    args = parser.parse_args(argv)
    if not (0 <= args.channel_threshold < 255 and 0 <= args.ink_threshold < 255):
        parser.error("channel and ink thresholds must be in 0..254")
    cli = args.cli.resolve()
    fixtures = read_fixtures(args.manifest, args.corpus_dir, set(args.fixture))
    font_files = [path.resolve() for path in args.font]
    explicit_fonts = [
        {"file": str(path), "sha256": sha256(path)} for path in font_files
    ]
    options = {
        key: getattr(args, key)
        for key in ("channel_threshold", "ink_threshold", "tolerance")
    }
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {
        "schema_version": 1,
        "options": options,
        "environment": {
            "python": platform.python_version(),
            "platform": platform.platform(),
            "pymupdf": pymupdf.__version__,
            "pillow": PIL.__version__,
            "numpy": np.__version__,
            "sdk_version": run([cli, "--version"]).stdout.strip(),
            "sdk_binary_sha256": sha256(cli),
            "workspace_git_head": run(
                ["git", "rev-parse", "HEAD"], cwd=ROOT
            ).stdout.strip(),
            "workspace_dirty": bool(
                run(["git", "status", "--porcelain"], cwd=ROOT).stdout
            ),
            "fontconfig": font_inventory(),
            "explicit_fonts": explicit_fonts,
        },
        "fixtures": [
            compare_fixture(fixture, cli, output, options, font_files, args.format)
            for fixture in fixtures
        ],
    }
    (output / "report.json").write_text(
        json.dumps(report, indent=2, allow_nan=False) + "\n"
    )
    write_html(report, output / "index.html")
    for fixture in report["fixtures"]:
        for page in fixture["pages"]:
            metrics = page["metrics"]
            assert all(math.isfinite(value) for value in metrics.values())
            print(
                f"{fixture['id']} page {page['page']}: changed {metrics['changed_pixel_fraction']:.2%}, "
                f"missing ink {metrics['missing_ink_fraction']:.2%}, extra ink {metrics['extra_ink_fraction']:.2%}"
            )
    print(f"Report: {output / 'index.html'}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"Visual comparison failed: {error}", file=sys.stderr)
        sys.exit(1)
