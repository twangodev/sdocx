#!/usr/bin/env python3
"""Run the retained v208 comparator with the observed PDFium 150 version."""
from pathlib import Path

script = Path(__file__).resolve().parent.parent / "simplebrush-v208/pdfium.py"
source = script.read_text()
old = 'assert str(pypdfium2.PDFIUM_INFO) == "156.0.8076.0"'
assert source.count(old) == 1
exec(compile(source.replace(old, 'assert str(pypdfium2.PDFIUM_INFO) == "150.0.7869.0"'), str(script), "exec"))
