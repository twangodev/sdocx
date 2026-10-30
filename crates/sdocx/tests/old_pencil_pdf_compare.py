"""Compare direct PDF output with pinned native host frames using PyMuPDF 1.28.2.

Run the ignored Rust test with SDOCX_PDF_ORACLE_PYTHON pointing to a Python
where PyMuPDF 1.28.2 is installed. The PDFium oracle additionally requires
pypdfium2 5.9.0, PDFium 150.0.7869.0, and Pillow (not version-pinned).
The Rust test supplies paths.
"""

import os
import re
import struct
import sys
import zlib
from pathlib import Path

import pymupdf

WIDTH, HEIGHT = 1920, 1080
RGBA_BYTES = WIDTH * HEIGHT * 4
WHITE_PAGE = re.compile(
    rb"q 0\.75 0 0 -0\.75 0 810 cm 1 g 0 0 m 1920 0 l "
    rb"1920 1080 l 0 1080 l\nh\nf\nQ\n"
)


def inflate(path: Path) -> bytes:
    pixels = zlib.decompress(path.read_bytes())
    assert len(pixels) == RGBA_BYTES
    return pixels


def flip_rows(pixels: bytes) -> bytes:
    stride = WIDTH * 4
    return b"".join(pixels[y * stride : (y + 1) * stride] for y in range(HEIGHT - 1, -1, -1))


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def rgba_png(pixels: bytes) -> bytes:
    rows = b"".join(
        b"\0" + pixels[y * WIDTH * 4 : (y + 1) * WIDTH * 4]
        for y in range(HEIGHT)
    )
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(rows))
        + chunk(b"IEND", b"")
    )


def render_with_prior(pdf_path: Path, prior_png: bytes, scale: int) -> bytes:
    document = pymupdf.open(pdf_path)
    page = document[0]
    content_id = page.get_contents()[0]
    content, matches = WHITE_PAGE.subn(b"", document.xref_stream(content_id))
    assert matches == 1, "the synthetic white page paint must be uniquely identifiable"
    if scale == 2:
        # Host projection intervention: scale about GL bottom-left after y flip.
        content = b"q 2 0 0 2 0 -810 cm\n" + content + b"\nQ\n"
    document.update_stream(content_id, content)
    page.insert_image(page.rect, stream=prior_png, overlay=False)
    pixmap = page.get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=False)
    assert (pixmap.width, pixmap.height, len(pixmap.samples)) == (WIDTH, HEIGHT, WIDTH * HEIGHT * 3)
    return pixmap.samples


def render_with_pdfium(pdf_path: Path, prior_png: bytes, scale: int, blank: bool = False) -> bytes:
    import pypdfium2

    document = pymupdf.open(pdf_path)
    page = document[0]
    content_id = page.get_contents()[0]
    content, matches = WHITE_PAGE.subn(b"", document.xref_stream(content_id))
    assert matches == 1
    if blank:
        content = b""
    elif scale == 2:
        content = b"q 2 0 0 2 0 -810 cm\n" + content + b"\nQ\n"
    document.update_stream(content_id, content)
    page.insert_image(page.rect, stream=prior_png, overlay=False)
    pdf_bytes = document.tobytes(garbage=3, deflate=True)
    document.close()
    pdf = pypdfium2.PdfDocument(pdf_bytes)
    image = pdf[0].render(scale=4 / 3).to_pil().convert("RGB")
    assert image.size == (WIDTH, HEIGHT)
    pixels = image.tobytes()
    assert len(pixels) == WIDTH * HEIGHT * 3
    return pixels


def delta_metrics(actual: bytes, native: bytes, prior: bytes, reader_prior: bytes) -> tuple[int, int, int]:
    union = [
        i for i in range(WIDTH * HEIGHT)
        if actual[3 * i : 3 * i + 3] != reader_prior[3 * i : 3 * i + 3]
        or native[4 * i : 4 * i + 4] != prior[4 * i : 4 * i + 4]
    ]
    errors = [
        abs((actual[3 * i + c] - reader_prior[3 * i + c]) - (native[4 * i + c] - prior[4 * i + c]))
        for i in union for c in range(3)
    ]
    return len(union), sum(errors), max(errors)


def error_metrics(actual: bytes, native: bytes, prior: bytes) -> tuple[int, int, int]:
    union = []
    for i in range(WIDTH * HEIGHT):
        if actual[3 * i : 3 * i + 3] != prior[4 * i : 4 * i + 3] or native[4 * i : 4 * i + 4] != prior[4 * i : 4 * i + 4]:
            union.append(i)
    errors = [abs(actual[3 * i + c] - native[4 * i + c]) for i in union for c in range(3)]
    return len(union), sum(errors), max(errors)


def straight_alpha_png(premultiplied: bytes) -> bytes:
    """Convert the native premultiplied prior to a straight-alpha PDF image."""
    straight = bytearray(premultiplied)
    tables = {
        alpha: [min(255, (value * 255 + alpha // 2) // alpha) if alpha else 0 for value in range(256)]
        for alpha in set(premultiplied[3::4])
    }
    for i in range(0, len(straight), 4):
        table = tables[straight[i + 3]]
        for channel in range(3):
            straight[i + channel] = table[straight[i + channel]]
    return rgba_png(bytes(straight))


def render_transparent_pdf(pdf_path: Path, prior_png: bytes, scale: int, reader: str, blank: bool = False) -> bytes:
    from PIL import Image

    document = pymupdf.open(pdf_path)
    page = document[0]
    content_id = page.get_contents()[0]
    content, matches = WHITE_PAGE.subn(b"", document.xref_stream(content_id))
    assert matches == 1
    if blank:
        content = b""
    elif scale == 2:
        content = b"q 2 0 0 2 0 -810 cm\n" + content + b"\nQ\n"
    document.update_stream(content_id, content)
    page.insert_image(page.rect, stream=prior_png, overlay=False)
    if reader == "mupdf":
        pixmap = page.get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=True)
        assert (pixmap.width, pixmap.height, len(pixmap.samples)) == (WIDTH, HEIGHT, RGBA_BYTES)
        return pixmap.samples
    import pypdfium2

    pdf = pypdfium2.PdfDocument(document.tobytes(garbage=3, deflate=True))
    image = pdf[0].render(scale=4 / 3, fill_color=(0, 0, 0, 0), rev_byteorder=True).to_pil()
    assert image.mode == "RGBA" and image.size == (WIDTH, HEIGHT)
    # PDFium supplies straight RGBA; MuPDF and the native frame are premultiplied.
    result = image.convert("RGBa").tobytes()
    assert len(result) == RGBA_BYTES
    return result


def transparent_metrics(actual: bytes, reader_prior: bytes, native: bytes, prior: bytes):
    from PIL import Image, ImageChops

    assert all(len(data) == RGBA_BYTES for data in (actual, reader_prior, native, prior))
    def changed_bounds(left: bytes, right: bytes):
        diff = ImageChops.difference(Image.frombytes("RGBA", (WIDTH, HEIGHT), left), Image.frombytes("RGBA", (WIDTH, HEIGHT), right))
        boxes = [box for channel in diff.split() if (box := channel.getbbox())]
        if not boxes:
            return None
        return (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
    boxes = [box for pair in ((actual, reader_prior), (native, prior)) if (box := changed_bounds(*pair))]
    assert boxes
    left, top = min(b[0] for b in boxes), min(b[1] for b in boxes)
    right, bottom = max(b[2] for b in boxes), max(b[3] for b in boxes)
    absolute_alpha_max = ImageChops.difference(
        Image.frombytes("RGBA", (WIDTH, HEIGHT), actual),
        Image.frombytes("RGBA", (WIDTH, HEIGHT), native),
    ).getchannel("A").getextrema()[1]
    sums, maxima, union = [0] * 4, [0] * 4, 0
    for y in range(top, bottom):
        for x in range(left, right):
            i = 4 * (y * WIDTH + x)
            if actual[i:i + 4] == reader_prior[i:i + 4] and native[i:i + 4] == prior[i:i + 4]:
                continue
            union += 1
            for channel in range(4):
                error = abs((actual[i + channel] - reader_prior[i + channel]) - (native[i + channel] - prior[i + channel]))
                sums[channel] += error
                maxima[channel] = max(maxima[channel], error)
    return union, sums, maxima, absolute_alpha_max


def check_overlap_2d(fixtures: Path, pdfs: Path, reader: str) -> None:
    directory = fixtures / "old-pencil-counterexample"
    # GL readback is bottom-origin; the capture projection maps those rows to
    # PDF top-origin rows. Stored fixtures are top-origin, so reverse once.
    prior = flip_rows(inflate(directory / "overlap-2d-pre.z"))
    mid = flip_rows(inflate(directory / "overlap-2d-mid.z"))
    native = flip_rows(inflate(directory / "overlap-2d-post.z"))
    assert prior != flip_rows(prior) and mid != prior and native != mid
    from collections import Counter
    for start, end, counts in [(prior, mid, {(51, 102, 153): 373, (26, 178, 76): 47}),
                                (mid, native, {(51, 102, 153): 196, (26, 178, 76): 216})]:
        changed = Counter(tuple(prior[i : i + 3]) for i in range(0, len(prior), 4)
                          if start[i : i + 4] != end[i : i + 4])
        assert changed == counts
    png = rgba_png(prior)
    def pixel(data: bytes, x: int, y: int, channels: int) -> tuple[int, ...]:
        i = channels * (y * WIDTH + x)
        return tuple(data[i : i + 3])
    assert [pixel(prior, *xy, 4) for xy in [(2, 10), (50, 10), (120, 220), (140, 220), (140, 210)]] == [
        (204, 38, 89), (51, 102, 153), (51, 102, 153), (26, 178, 76), (51, 102, 153)
    ]
    # Profile page invokes two forms; fallback draws neither as a form.
    for enabled in [False, True]:
        pdf = pymupdf.open(pdfs / f"overlap_2d-{str(enabled).lower()}.pdf")
        page_content = pdf.xref_stream(pdf[0].get_contents()[0])
        assert page_content.count(b" Do\n") == (2 if enabled else 0)
    blank = render_with_pdfium(pdfs / "overlap_2d-false.pdf", png, 1, blank=True) if reader == "pdfium" else None
    if blank is not None:
        # PDFium's image conversion rounds channels, but preserves region placement.
        assert [pixel(blank, *xy, 3) for xy in [(2, 10), (50, 10), (120, 220), (140, 220), (140, 210)]] == [
            (200, 35, 85), (50, 100, 150), (50, 100, 150), (25, 175, 75), (50, 100, 150)
        ]
    else:
        pdf = pymupdf.open(pdfs / "overlap_2d-false.pdf")
        page = pdf[0]
        pdf.update_stream(page.get_contents()[0], b"")
        page.insert_image(page.rect, stream=png, overlay=False)
        raster = page.get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=False).samples
        assert raster == bytes(v for i, v in enumerate(prior) if i % 4 != 3)
    expected = [(773, 76998, 213), (865, 30427, 110)] if reader == "pdfium" else [(773, 75834, 210), (881, 28942, 77)]
    for enabled, metric in zip([False, True], expected, strict=True):
        path = pdfs / f"overlap_2d-{str(enabled).lower()}.pdf"
        if reader == "pdfium":
            measured = delta_metrics(render_with_pdfium(path, png, 1), native, prior, blank)
        else:
            measured = error_metrics(render_with_prior(path, png, 1), native, prior)
        assert measured == metric, (reader, enabled, measured, metric)


def main() -> None:
    assert pymupdf.VersionBind == "1.28.2", "the oracle requires PyMuPDF 1.28.2"
    fixtures, pdfs = map(Path, sys.argv[1:3])
    reader = sys.argv[3]
    prior = flip_rows(inflate(fixtures / "old-pencil-color/varied-pre.rgba.zlib"))
    assert all(alpha == 255 for alpha in prior[3::4])
    prior_png = rgba_png(prior)
    cases = [
        ("blue", 1, "serialized-pencil-color-post.rgba.zlib", (423, 23239, 59), (489, 11557, 33)),
        ("blue", 2, "serialized_color_scale2-top.rgba.zlib", (1661, 96975, 63), (1838, 38008, 42)),
        ("alt", 1, "serialized-pencil-color-alt-post.rgba.zlib", (420, 40607, 185), (489, 14285, 64)),
        ("reflect", 1, "serialized8-reflect-1.rgba.zlib", (360, 19724, 63), (444, 10403, 39)),
        ("reflect", 2, "serialized8-reflect-2.rgba.zlib", (1452, 84217, 63), (1630, 35459, 34)),
    ]
    cases.append(("overlap", 1, "overlap-post.z", (423, 25308, 62), (489, 11465, 47)))
    if reader == "pdfium":
        import pypdfium2

        assert str(pypdfium2.PYPDFIUM_INFO) == "5.9.0"
        assert str(pypdfium2.PDFIUM_INFO) == "150.0.7869.0"
        cases = [
            ("blue", 1, "serialized-pencil-color-post.rgba.zlib", (423, 23586, 62), (485, 11654, 51)),
            ("blue", 2, "serialized_color_scale2-top.rgba.zlib", (1661, 99178, 66), (1814, 35208, 49)),
            ("alt", 1, "serialized-pencil-color-alt-post.rgba.zlib", (420, 41503, 186), (481, 14787, 98)),
            ("reflect", 1, "serialized8-reflect-1.rgba.zlib", (360, 20051, 66), (438, 12819, 52)),
            ("reflect", 2, "serialized8-reflect-2.rgba.zlib", (1452, 85951, 66), (1609, 33551, 48)),
        ]
        cases.append(("overlap", 1, "overlap-post.z", (423, 25476, 64), (485, 11978, 58)))
        reader_prior = render_with_pdfium(pdfs / "blue-false.pdf", prior_png, 1, blank=True)
    else:
        assert reader == "mupdf"
    for name, scale, fixture, fallback, candidate in cases:
        if name == "overlap":
            assert candidate[1] < fallback[1]
        native = flip_rows(inflate(fixtures / "old-pencil-counterexample" / fixture))
        for enabled, expected in [(False, fallback), (True, candidate)]:
            pdf_path = pdfs / f"{name}-{str(enabled).lower()}.pdf"
            if reader == "pdfium":
                actual = render_with_pdfium(pdf_path, prior_png, scale)
                measured = delta_metrics(actual, native, prior, reader_prior)
            else:
                actual = render_with_prior(pdf_path, prior_png, scale)
                measured = error_metrics(actual, native, prior)
            assert measured == expected, (name, scale, enabled, measured, expected)


    check_overlap_2d(fixtures, pdfs, reader)

    # Size-12 synthetic controls; 2x is a host projection intervention.
    size12 = [
        ("plain", 1, (546,74930,183), (630,33841,109), (546,74741,184), (616,33920,138)),
        ("plain", 2, (2174,322489,190), (2393,108295,111), (2175,321947,191), (2365,103668,132)),
        ("reflect", 1, (553,62314,202), (632,19329,73), (553,63914,206), (614,19100,93)),
        ("reflect", 2, (2181,250686,203), (2394,63866,64), (2181,255207,206), (2355,58621,79)),
        ("rapid", 1, (765,72000,162), (872,41396,126), (765,71657,163), (865,37792,150)),
        ("rapid", 2, (3116,291820,162), (3343,123967,133), (3117,291668,163), (3319,113671,143)),
    ]
    directory = fixtures / "old-pencil-counterexample"
    for case, scale, mu_fallback, mu_profile, pi_fallback, pi_profile in size12:
        prior_case = "reflect" if case == "reflect" else "plain"
        prior = flip_rows(inflate(directory / f"serialized5-{prior_case}-pre.z"))
        native_file = f"rapid-bend-{scale}.z" if case == "rapid" else f"interval-12_{case}-{scale}.z"
        native = flip_rows(inflate(directory / native_file))
        png = rgba_png(prior)
        baseline = render_with_pdfium(pdfs / f"size12_{case}-false.pdf", png, 1, blank=True) if reader == "pdfium" else None
        for enabled in [False, True]:
            pdf_path = pdfs / f"size12_{case}-{str(enabled).lower()}.pdf"
            if reader == "pdfium":
                measured = delta_metrics(render_with_pdfium(pdf_path, png, scale), native, prior, baseline)
                expected = pi_profile if enabled else pi_fallback
            else:
                measured = error_metrics(render_with_prior(pdf_path, png, scale), native, prior)
                expected = mu_profile if enabled else mu_fallback
            assert measured == expected, (case, scale, enabled, measured, expected)

    # The visible size-12 single-stamp dot stays on fallback.
    dot_prior = flip_rows(inflate(directory / "serialized5-plain-pre.z"))
    dot_png = rgba_png(dot_prior)
    baseline = render_with_pdfium(pdfs / "size12_dot-false.pdf", dot_png, 1, blank=True) if reader == "pdfium" else None
    for scale, mu, pi in [(1, (28,3861,190), (28,3912,191)), (2, (106,19485,190), (109,19830,191))]:
        native = flip_rows(inflate(directory / f"dot-12-visible-{scale}.z"))
        assert dot_prior == flip_rows(dot_prior)
        changed = [i for i in range(WIDTH * HEIGHT) if native[4*i:4*i+4] != dot_prior[4*i:4*i+4]]
        bounds = (min(i % WIDTH for i in changed), min(i // WIDTH for i in changed), max(i % WIDTH for i in changed), max(i // WIDTH for i in changed))
        assert bounds == ((130,208,135,213) if scale == 1 else (260,417,271,428))
        for enabled in [False, True]:
            pdf_path = pdfs / f"size12_dot-{str(enabled).lower()}.pdf"
            if reader == "pdfium":
                actual = delta_metrics(render_with_pdfium(pdf_path, dot_png, scale), native, dot_prior, baseline)
                expected = pi
            else:
                pixels = render_with_prior(pdf_path, dot_png, scale)
                pdf_changed = [i for i in range(WIDTH * HEIGHT) if pixels[3*i:3*i+3] != dot_prior[4*i:4*i+3]]
                pdf_bounds = (min(i % WIDTH for i in pdf_changed), min(i // WIDTH for i in pdf_changed), max(i % WIDTH for i in pdf_changed), max(i // WIDTH for i in pdf_changed))
                assert pdf_bounds == ((130,208,135,213) if scale == 1 else (261,417,270,427)), (scale, pdf_bounds)
                actual = error_metrics(pixels, native, dot_prior)
                expected = mu
            assert actual == expected, (scale, enabled, actual, expected)

    # Independent one-cell geometry isolates PDF reader coverage from ink data.
    for scale, target, label in [(1, 50.5, "center"), (1, 50.25, "quarter"), (2, 50.5, "center"), (2, 50.25, "quarter")]:
        pdf_path = pdfs / f"onehot-{scale}-{label}.pdf"
        pixel = int(target * scale)
        if reader == "mupdf":
            document = pymupdf.open(pdf_path)
            pixmap = document[0].get_pixmap(matrix=pymupdf.Matrix(scale * 4 / 3, scale * 4 / 3), alpha=True)
            assert (pixmap.width, pixmap.height) == (100 * scale, 100 * scale)
            alpha = pixmap.samples[4 * (pixel * pixmap.width + pixel) + 3]
        else:
            document = pypdfium2.PdfDocument(str(pdf_path))
            image = document[0].render(scale=scale * 4 / 3, fill_color=(0, 0, 0, 0), rev_byteorder=True).to_pil()
            assert image.mode == "RGBA" and image.size == (100 * scale, 100 * scale)
            alpha = image.getpixel((pixel, pixel))[3]
        start = target - (0.5 / 48.0) * 7.0
        end = start + 7.0 / 48.0
        expected_center = 255 if start <= (pixel + 0.5) / scale < end else 0
        reference = {(1, "center"): 255, (1, "quarter"): 0, (2, "center"): 0, (2, "quarter"): 255}[(scale, label)]
        reader_alpha = {
            ("mupdf", 1, "center"): 3, ("mupdf", 1, "quarter"): 3,
            ("mupdf", 2, "center"): 3, ("mupdf", 2, "quarter"): 15,
            ("pdfium", 1, "center"): 255, ("pdfium", 1, "quarter"): 255,
            ("pdfium", 2, "center"): 0, ("pdfium", 2, "quarter"): 255,
        }[(reader, scale, label)]
        assert expected_center == reference and alpha == reader_alpha, (reader, scale, label, expected_center, alpha)

    # Size 5 remains on the public PDF fallback for both option values.
    size5_cases = [
        ("plain", 1, (230, 27723, 189), (230, 28154, 190), (302, 18934, 113), (294, 24170, 163)),
        ("plain", 2, (898, 127679, 190), (899, 127966, 191), (1079, 60641, 117), (1051, 62423, 149)),
        ("reflect", 1, (232, 24505, 195), (232, 26165, 202), (302, 9905, 77), (281, 14076, 121)),
        ("reflect", 2, (903, 100159, 203), (903, 102729, 206), (1080, 32935, 72), (1047, 34820, 101)),
    ]
    diagnostic = os.environ.get("SDOCX_PENCIL_SIZE5_DIAGNOSTIC") == "1"
    for name, scale, mupdf_expected, pdfium_expected, mupdf_candidate, pdfium_candidate in size5_cases:
        prior = flip_rows(inflate(fixtures / "old-pencil-counterexample" / f"serialized5-{name}-pre.z"))
        assert all(alpha == 255 for alpha in prior[3::4])
        prior_png = rgba_png(prior)
        native = flip_rows(inflate(fixtures / "old-pencil-counterexample" / f"serialized5-{name}-{scale}.z"))
        if reader == "pdfium":
            reader_prior = render_with_pdfium(pdfs / f"size5_{name}-false.pdf", prior_png, 1, blank=True)
        for enabled in [False, True]:
            pdf_path = pdfs / f"size5_{name}-{str(enabled).lower()}.pdf"
            if reader == "pdfium":
                actual = render_with_pdfium(pdf_path, prior_png, scale)
                measured = delta_metrics(actual, native, prior, reader_prior)
                expected = pdfium_candidate if diagnostic and enabled else pdfium_expected
            else:
                actual = render_with_prior(pdf_path, prior_png, scale)
                measured = error_metrics(actual, native, prior)
                expected = mupdf_candidate if diagnostic and enabled else mupdf_expected
            assert measured == expected, (name, scale, enabled, measured, expected)


    alpha_cases = [
        ("clear", 1, "transparent-clear-pre.z", (422,[6134,27330,36919,43609],[34,152,205,242]), (479,[3268,13416,18000,21317],[18,89,119,141]), (423,[6121,27202,36733,43353],[34,153,206,243]), (485,[3026,13511,18219,21558],[28,127,170,201])),
        ("clear", 2, "transparent-clear-pre.z", (1661,[25652,114275,154328,182270],[36,160,216,255]), (1813,[10720,43946,58873,69760],[23,108,144,174]), (1661,[25700,114181,154169,182029],[36,160,216,255]), (1814,[9333,41710,56327,66944],[28,123,166,196])),
        ("translucent", 1, "transparent-translucent1-pre.z", (423,[2135,22034,28332,27349],[15,137,178,180]), (482,[1080,10542,13659,13336],[9,70,91,93]), (423,[2190,21819,28216,27143],[15,137,178,180]), (485,[1050,10619,13778,13324],[10,98,127,129])),
        ("translucent", 2, "transparent-translucent2-pre.z", (1661,[8882,91885,118135,113856],[15,148,191,192]), (1821,[3532,34736,45002,44193],[11,91,119,121]), (1661,[9023,91443,118216,113451],[15,147,191,191]), (1814,[3602,33424,43029,41836],[12,111,146,148])),
    ]
    for name, scale, pre_fixture, mu_fallback, mu_profile, pdfium_fallback, pdfium_profile in alpha_cases:
        directory = fixtures / "old-pencil-counterexample"
        prior = flip_rows(inflate(directory / pre_fixture))
        native = flip_rows(inflate(directory / f"transparent-{name}{scale}-post.z"))
        prior_png = straight_alpha_png(prior)
        baseline = render_transparent_pdf(pdfs / "blue-false.pdf", prior_png, scale, reader, blank=True)
        for enabled, expected in [(False, mu_fallback if reader == "mupdf" else pdfium_fallback), (True, mu_profile if reader == "mupdf" else pdfium_profile)]:
            actual = render_transparent_pdf(pdfs / f"blue-{str(enabled).lower()}.pdf", prior_png, scale, reader)
            if name == "clear" and enabled:
                # Fixtures store GL readback in top-origin rows. flip_rows maps
                # native top y=854 to PDF page y=225 (1x); the 2x host
                # projection maps the landmark to page (264, 415).
                x, y, native_pixel, reader_pixels = (
                    (130, 225, bytes([2, 7, 10, 12]), {"mupdf": bytes([20, 96, 129, 153]), "pdfium": bytes([30, 134, 180, 213])})
                    if scale == 1
                    else (264, 415, bytes([3, 15, 21, 23]), {"mupdf": bytes([26, 123, 165, 197]), "pdfium": bytes([28, 124, 168, 198])})
                )
                at = 4 * (y * WIDTH + x)
                assert native[at:at + 4] == native_pixel
                assert actual[at:at + 4] == reader_pixels[reader]
            measured = transparent_metrics(actual, baseline, native, prior)
            # Both reader priors preserve the native alpha in these controls.
            absolute_alpha = {
                ("mupdf", "translucent", 1, False): 181,
                ("mupdf", "translucent", 1, True): 94,
                ("mupdf", "translucent", 2, False): 191,
                ("mupdf", "translucent", 2, True): 120,
            }.get((reader, name, scale, enabled), expected[2][3])
            assert measured == (*expected, absolute_alpha), (reader, name, scale, enabled, measured, expected)


if __name__ == "__main__":
    main()
