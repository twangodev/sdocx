"""Small synthetic checks for the visual runner, independent of corpus files."""

from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image, ImageDraw
import pymupdf

import visual


class VisualTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def ink_image(self, x=10):
        image = Image.new("RGB", (100, 100), "white")
        ImageDraw.Draw(image).rectangle((x, 10, x + 9, 19), fill="black")
        return image

    def fixture(self):
        source = self.root / "source.sdocx"
        source.write_bytes(b"synthetic archive placeholder")
        reference = self.root / "reference.pdf"
        with pymupdf.open() as pdf:
            page = pdf.new_page(width=100, height=100)
            page.draw_rect(pymupdf.Rect(10, 10, 20, 20), fill=(0, 0, 0))
            pdf.save(reference)
        fields = [
            "fixture",
            source.name,
            visual.sha256(source),
            reference.name,
            visual.sha256(reference),
            "1",
            "1",
        ] + [""] * 10
        manifest = self.root / "corpus.tsv"
        manifest.write_text("\t".join(fields) + "\n")
        return manifest

    def test_identical_blank_and_transparent_images(self):
        for reference, candidate in (
            (self.ink_image(), self.ink_image()),
            (Image.new("RGB", (100, 100), "white"), Image.new("RGBA", (100, 100))),
        ):
            metrics, difference = visual.measure(reference, candidate)
            self.assertEqual(metrics["changed_pixel_fraction"], 0)
            self.assertEqual(metrics["missing_ink_fraction"], 0)
            self.assertEqual(metrics["extra_ink_fraction"], 0)
            self.assertIsNone(difference.getbbox())

    def test_blank_output_cannot_hide_behind_white_page_area(self):
        metrics, _ = visual.measure(
            self.ink_image(), Image.new("RGB", (100, 100), "white")
        )
        self.assertAlmostEqual(metrics["changed_pixel_fraction"], 0.01)
        self.assertEqual(metrics["missing_ink_fraction"], 1)
        self.assertEqual(metrics["sdk_ink_pixels"], 0)
        metrics, _ = visual.measure(
            Image.new("RGB", (100, 100), "white"), self.ink_image()
        )
        self.assertEqual(metrics["extra_ink_fraction"], 1)

    def test_ink_tolerance_does_not_align_away_layout_changes(self):
        exact, _ = visual.measure(self.ink_image(), self.ink_image(11), tolerance=0)
        tolerant, _ = visual.measure(self.ink_image(), self.ink_image(11), tolerance=1)
        shifted, _ = visual.measure(self.ink_image(), self.ink_image(30), tolerance=1)
        self.assertGreater(exact["missing_ink_fraction"], 0)
        self.assertEqual(tolerant["missing_ink_fraction"], 0)
        self.assertEqual(shifted["missing_ink_fraction"], 1)
        self.assertEqual(
            exact["changed_pixel_fraction"], tolerant["changed_pixel_fraction"]
        )

    def test_dark_to_light_rgb_differences_do_not_overflow(self):
        metrics, difference = visual.measure(
            Image.new("RGB", (1, 1), "white"), Image.new("RGB", (1, 1), "black")
        )
        self.assertEqual(metrics["mean_absolute_rgb_error"], 1)
        self.assertEqual(difference.getpixel((0, 0)), (255, 255, 255))
        with self.assertRaisesRegex(ValueError, "matching dimensions"):
            visual.measure(Image.new("RGB", (1, 1)), Image.new("RGB", (2, 1)))

    def test_pdf_rotation_and_size_normalization_preserve_aspect_ratio(self):
        with pymupdf.open() as pdf:
            page = pdf.new_page(width=100, height=200)
            page.set_rotation(90)
            self.assertEqual(
                visual.rasterize_reference(page, (400, 200)).size, (400, 200)
            )
            with self.assertRaisesRegex(ValueError, "aspect ratio"):
                visual.rasterize_reference(page, (400, 400))

    def test_manifest_checks_hashes_selection_and_unique_ids(self):
        manifest = self.fixture()
        self.assertEqual(len(visual.read_fixtures(manifest, self.root, {"fixture"})), 1)
        with self.assertRaisesRegex(ValueError, "unknown fixture"):
            visual.read_fixtures(manifest, self.root, {"missing"})
        original = manifest.read_text()
        manifest.write_text(original * 2)
        with self.assertRaisesRegex(ValueError, "duplicate fixture"):
            visual.read_fixtures(manifest, self.root, set())
        manifest.write_text(original)
        (self.root / "source.sdocx").write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            visual.read_fixtures(manifest, self.root, set())

    def test_manifest_cannot_escape_the_corpus(self):
        manifest = self.fixture()
        manifest.write_text(
            manifest.read_text().replace("source.sdocx", "../source.sdocx")
        )
        with self.assertRaisesRegex(ValueError, "outside the corpus"):
            visual.read_fixtures(manifest, self.root, set())

    def test_page_order_is_numeric_and_extra_or_missing_pages_fail(self):
        for index in reversed(range(12)):
            (self.root / f"sdk_page{index}.png").touch()
        paths = visual.candidate_pages(self.root, 12)
        self.assertEqual(paths[2].name, "sdk_page2.png")
        self.assertEqual(paths[10].name, "sdk_page10.png")
        with self.assertRaisesRegex(ValueError, "page count"):
            visual.candidate_pages(self.root, 11)
        paths[5].unlink()
        with self.assertRaisesRegex(ValueError, "page count"):
            visual.candidate_pages(self.root, 12)

    def test_existing_output_is_rejected_before_running_cli(self):
        manifest = self.fixture()
        with patch.object(visual, "run") as run:
            with self.assertRaises(FileExistsError):
                visual.main(
                    [
                        "--manifest",
                        str(manifest),
                        "--corpus-dir",
                        str(self.root),
                        "--output",
                        str(self.root),
                    ]
                )
            run.assert_not_called()

    def test_fixture_render_produces_metrics_and_relative_artifacts(self):
        fixture = visual.read_fixtures(self.fixture(), self.root, set())[0]
        output = self.root / "output"
        output.mkdir()

        def fake_cli(command):
            self.ink_image().save(command[-1])
            return type("Result", (), {"stdout": "", "stderr": "diagnostic\n"})()

        with patch.object(visual, "run", side_effect=fake_cli):
            result = visual.compare_fixture(fixture, Path("fake-cli"), output, {})
        self.assertEqual(len(result["pages"]), 1)
        page = result["pages"][0]
        self.assertGreater(page["metrics"]["reference_ink_pixels"], 0)
        for key in ("reference", "sdk", "difference"):
            self.assertTrue((output / page[key]).is_file())
            self.assertFalse(Path(page[key]).is_absolute())
        visual.write_html({"fixtures": [result]}, output / "index.html")
        self.assertIn("SDK overlay opacity", (output / "index.html").read_text())
        self.assertEqual((output / "fixture/sdk.log").read_text(), "diagnostic\n")

    def test_pdf_page_count_mismatch_is_an_error(self):
        fixture = visual.read_fixtures(self.fixture(), self.root, set())[0]
        fixture["visible_pages"] = 2
        with patch.object(visual, "run") as run:
            with self.assertRaisesRegex(ValueError, "PDF page count"):
                visual.compare_fixture(fixture, Path("fake-cli"), self.root, {})
            run.assert_not_called()

    def test_explicit_fonts_are_forwarded_in_order(self):
        fixture = visual.read_fixtures(self.fixture(), self.root, set())[0]
        fonts = [self.root / "regular.ttf", self.root / "symbols.otf"]

        def fake_cli(command):
            self.assertEqual(command[-4:], ["--font", fonts[0], "--font", fonts[1]])
            self.ink_image().save(command[command.index("--output") + 1])
            return type("Result", (), {"stdout": "", "stderr": ""})()

        with patch.object(visual, "run", side_effect=fake_cli):
            visual.compare_fixture(fixture, Path("fake-cli"), self.root, {}, fonts)

    def test_pdf_export_records_dimensions_text_hash_and_download(self):
        fixture = visual.read_fixtures(self.fixture(), self.root, set())[0]

        def fake_cli(command):
            destination = command[command.index("--output") + 1]
            self.assertEqual(destination.suffix, ".pdf")
            with pymupdf.open() as pdf:
                page = pdf.new_page(width=75, height=75)
                page.insert_text((5, 20), "Selectable")
                pdf.save(destination)
            return type("Result", (), {"stdout": "", "stderr": ""})()

        with patch.object(visual, "run", side_effect=fake_cli):
            result = visual.compare_fixture(
                fixture, Path("fake-cli"), self.root, {}, output_format="pdf"
            )
        page = result["pages"][0]
        self.assertEqual(page["size"], [100, 100])
        self.assertEqual(page["sdk_pdf_size_points"], [75, 75])
        self.assertIn("Selectable", (self.root / page["sdk_text"]).read_text())
        self.assertEqual(
            result["sdk_pdf_sha256"], visual.sha256(self.root / result["sdk_pdf"])
        )
        visual.write_html({"fixtures": [result]}, self.root / "index.html")
        self.assertIn('href="fixture/sdk.pdf"', (self.root / "index.html").read_text())

    def test_candidate_pdf_count_and_page_order(self):
        path = self.root / "sdk.pdf"
        with pymupdf.open() as pdf:
            for index in range(12):
                page = pdf.new_page(width=75 + index * 3, height=75)
                page.insert_text((5, 20), str(index))
            pdf.save(path)
        with self.assertRaisesRegex(ValueError, "SDK PDF page count"):
            visual.rasterize_candidate_pdf(path, 11)
        pages = visual.rasterize_candidate_pdf(path, 12)
        self.assertEqual(len(visual.candidate_pages(self.root, 12)), 12)
        self.assertEqual(pages[10]["sdk_pdf_size_points"], [105, 75])
        self.assertEqual((self.root / pages[10]["sdk_text"]).read_text().strip(), "10")


if __name__ == "__main__":
    unittest.main()
