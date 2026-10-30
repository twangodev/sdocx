# Long Montblanc 2× controls

Two independent synthetic Drawing trajectories at size 1.5 exercise 10 and 11 points with varying pressure. The native callback streams are checked against the Rust sampler. Native host-Mesa frames use an explicitly supplied 2× projection; this is not evidence of saved-page zoom, device output, or native PDF export.

Against the same inputs, RGB absolute error sums for SVG/resvg fall from 463289 to 59479 and 366335 to 44674. Direct PDF MuPDF falls from 462019 to 48644 and 360970 to 35067; PDFium falls from 468294 to 47877 and 354915 to 34792. Both candidates overcover native support. SVG is 588–611 KB and PDF 128–143 KB per stroke. The receipt contains inputs, hashes, versions, and support metrics. Other shapes and sizes remain unverified.

Use two fresh empty directories. Run `compare.py capture` with `MB_EXPORT_WORK` set to the capture directory, then `compare.py rebuild-verify` with `MB_CAPTURE_WORK` set to that directory and `MB_EXPORT_WORK` set to the export directory. Requirements and partial runtime pinning match `../long-paint/README.md`; the interpreter also needs pypdfium2. The compact archive retains native frames and streams; verification may use the retained capture archive and requires fresh export. Never use a directory containing unrelated files.
