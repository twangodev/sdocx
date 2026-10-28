# Montblanc 2x four/five-point host controls

Run from the repository root with its existing Python environment:

```sh
.venv/bin/python tools/montblanc/extended-2x/verify.py --repo "$PWD" --capture
```

The verifier checks hashes for every packaged artifact, archives the immutable revision in manifest.json, checks selected source, fixture, lockfiles and patched-source hashes, applies gate.patch.gz, then builds public SVG/PDF output using Cargo --locked --offline. It verifies library versions, cropped native frame hashes and reconstructed full-frame hashes, reruns both native controls and compares their frame hashes, and asserts every output metric against the receipt. Omitting --capture skips native recapture but still checks retained crops and public output. Native recapture uses the repository's scratch native libraries and Python runtime adapters, the checked-in pinned harness, Unicorn and pinned Mesa EGL/GLES libraries; their library hashes are checked by the harness. The capture wrapper injects exactly the point/pressure controls in verify.py and a 2x matrix with translation (128,128); geometry is asserted against the retained native streams.

The gate uses the existing unit-scale `prepare` admission bounds for three to five points. Tests verify four/five-point entry into the 2x route, rejection for unsupported settings, replay, slice, and conflicting assertions. The comparison metrics are [native support, export support, intersection, union, RGB L1 on union], with native premultiplied RGBA composited once over #fcfcfc. The candidate reduces RGB error for these examples but overcovers. These synthetic host Mesa controls do not establish device, saved-note, replay, arbitrary-input, or general-scale parity.
