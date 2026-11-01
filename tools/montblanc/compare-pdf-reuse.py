#!/usr/bin/env python3
"""Compare paired flat/shared PDF test fixtures using MuPDF and PDFium."""
import argparse
import hashlib
from pathlib import Path

import pymupdf
import pypdfium2

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=Path)
args = parser.parse_args()
for case in ("", "-offscreen", "-mask"):
    for scale in (1, 2):
        for reader in ("mupdf", "pdfium"):
            hashes = []
            for kind in ("flat", "shared"):
                pdf = args.directory / f"{kind}{case}.pdf"
                if reader == "mupdf":
                    with pymupdf.open(pdf) as document:
                        data = document[0].get_pixmap(matrix=pymupdf.Matrix(scale, scale), alpha=False).samples
                else:
                    with pypdfium2.PdfDocument(pdf) as document:
                        page = document[0]
                        bitmap = page.render(scale=scale, rev_byteorder=True)
                        data = bitmap.to_pil().convert("RGB").tobytes()
                        bitmap.close()
                        page.close()
                hashes.append(hashlib.sha256(data).hexdigest())
            assert hashes[0] == hashes[1], (case, scale, reader, hashes)
            print(case or "mixed", scale, reader, hashes[0])
