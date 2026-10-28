# Experimental public Montblanc comparison

This synthetic probe sets `HostUnitScale` explicitly and compares public SVG and native PDF exports against retained host Mesa frames and the generic renderer. It assumes one output pixel per page unit and a fresh drawable per stroke. It does not establish saved-note, device, or arbitrary-transform parity.

Run `.venv/bin/python tools/montblanc/public-optin/compare.py` with offline Cargo dependencies, resvg, Pillow, and PyMuPDF. Generated exports are temporary. The native RGB is premultiplied; the comparison adds it to the paper contribution `252 * (1 - alpha)` instead of multiplying it by alpha again. The retained frame does not establish the native fixed-function blend state.
