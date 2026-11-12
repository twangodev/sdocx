"""Synthetic preparation/lifetime checks without installed PDF readers."""

import importlib.util
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch


WHITE_PAINT = (b"q 0.75 0 0 -0.75 0 810 cm 1 g 0 0 m 1920 0 l "
               b"1920 1080 l 0 1080 l\nh\nf\nQ\n")
INK = b"q 0.2 g 1 2 3 4 re f Q\n"


class SyntheticPage:
    rect = (0, 0, 1440, 810)

    def __init__(self):
        self.images = []
        self.insertion_error = None

    def get_contents(self):
        return [17]

    def insert_image(self, rect, *, stream, overlay):
        if self.insertion_error is not None:
            raise self.insertion_error
        self.images.append((rect, stream, overlay))


class SyntheticDocument:
    def __init__(self, content):
        self.content = content
        self.page = SyntheticPage()
        self.updated = []
        self.closed = False

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.closed = True

    def __getitem__(self, index):
        if index != 0:
            raise IndexError(index)
        return self.page

    def xref_stream(self, content_id):
        if content_id != 17:
            raise KeyError(content_id)
        return self.content

    def update_stream(self, content_id, content):
        self.updated.append((content_id, content))


class PreparedPdfTests(unittest.TestCase):
    def setUp(self):
        self.document = SyntheticDocument(WHITE_PAINT + INK)
        self.opened = []
        fake_reader = types.ModuleType("pymupdf")

        def open_document(path):
            self.opened.append(path)
            return self.document

        fake_reader.open = open_document
        helper = Path(__file__).with_name("old_pencil_pdf_compare.py")
        spec = importlib.util.spec_from_file_location("old_pencil_preparation_test", helper)
        self.module = importlib.util.module_from_spec(spec)
        with patch.dict(sys.modules, {"pymupdf": fake_reader}):
            spec.loader.exec_module(self.module)

    def test_removes_only_white_paint_and_inserts_prior_before_yielding(self):
        self.document.content = INK + WHITE_PAINT + INK
        path = Path("supplied.pdf")
        with self.module.prepared_pdf_page(path, b"prior PNG", 1) as (document, page):
            self.assertIs(document, self.document)
            self.assertIs(page, self.document.page)
            self.assertEqual(self.opened, [path])
            self.assertEqual(document.updated, [(17, INK + INK)])
            self.assertEqual(page.images, [(page.rect, b"prior PNG", False)])
            self.assertFalse(document.closed)
        self.assertTrue(self.document.closed)

    def test_scale_two_uses_exact_bottom_left_projection_bytes(self):
        with self.module.prepared_pdf_page(Path("supplied.pdf"), b"prior", 2):
            self.assertEqual(self.document.updated, [(17, b"q 2 0 0 2 0 -810 cm\n" + INK + b"\nQ\n")])
        self.assertTrue(self.document.closed)

    def test_blank_takes_precedence_over_scale_and_still_inserts_prior(self):
        with self.module.prepared_pdf_page(Path("supplied.pdf"), b"prior", 2, blank=True):
            self.assertEqual(self.document.updated, [(17, b"")])
            self.assertEqual(self.document.page.images, [(self.document.page.rect, b"prior", False)])

    def test_reader_failure_closes_prepared_document(self):
        with self.assertRaisesRegex(RuntimeError, "reader failed"):
            with self.module.prepared_pdf_page(Path("supplied.pdf"), b"prior", 1):
                raise RuntimeError("reader failed")
        self.assertTrue(self.document.closed)

    def test_missing_or_duplicated_white_paint_is_rejected_even_for_blank(self):
        for content in (INK, WHITE_PAINT + WHITE_PAINT + INK):
            with self.subTest(content=content):
                self.document = SyntheticDocument(content)
                with self.assertRaisesRegex(AssertionError, "uniquely identifiable"):
                    with self.module.prepared_pdf_page(Path("supplied.pdf"), b"prior", 2, blank=True):
                        self.fail("invalid preparation was yielded")
                self.assertTrue(self.document.closed)
                self.assertEqual(self.document.updated, [])
                self.assertEqual(self.document.page.images, [])

    def test_prior_insertion_failure_closes_document(self):
        self.document.page.insertion_error = RuntimeError("prior failed")
        with self.assertRaisesRegex(RuntimeError, "prior failed"):
            with self.module.prepared_pdf_page(Path("supplied.pdf"), b"prior", 1):
                self.fail("failed insertion was yielded")
        self.assertTrue(self.document.closed)


if __name__ == "__main__":
    unittest.main()
