# AirBrush V1 curved host control

The retained synthetic Model/Drawing host-Mesa control matches public SVG exactly. Direct PDF RGBA error is 1,797 versus 18,764 for generic output in PyMuPDF 1.28.2 at 96 dpi. These measurements do not establish device, saved-note, arbitrary-scale or cross-reader parity.

The verifier requires the recorded historical Rust tree, not current HEAD. From this repository, reconstruct it in a disposable directory; the regression added by `d234c521` must be inserted into the earlier `e4f2c39f6007910f07cd526bd0fb89bb17f380a2` source. The following copies only its added test, preserving the receipt's source digest `e8a8b4485b6b3c85dca231d6dffd07a8f624d26f10ab9b251f10570ea49b7bc4`:

```sh
python3 - <<'PY_REPLAY'
import io
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

root = Path.cwd()
work = Path(tempfile.mkdtemp(prefix="sdocx-airbrush-curved-", dir="/tmp"))
archive = subprocess.check_output([
    "git", "archive", "e4f2c39f6007910f07cd526bd0fb89bb17f380a2"
])
with tarfile.open(fileobj=io.BytesIO(archive)) as stream:
    stream.extractall(work, filter="data")
patch = subprocess.check_output([
    "git", "show", "--format=", "d234c521", "--", "crates/sdocx/src/render.rs"
], text=True)
addition = "".join(line[1:] for line in patch.splitlines(keepends=True)
                   if line.startswith("+") and not line.startswith("+++"))
source = work / "crates/sdocx/src/render.rs"
anchor = ('    #[cfg(feature = "pdf")]\n    #[test]\n'
          '    fn saved_v1_public_short_stroke_matches_native_frame()')
text = source.read_text()
assert text.count(anchor) == 1
assert "fn saved_v1_curved_public_exports_match_native_control()" in addition
source.write_text(text.replace(anchor, addition + anchor, 1))
for relative in (
    "conformance/airbrush-v1-curved-export-control.json",
    "tools/pen-capture/airbrush_v1_curved_capture.py",
    "tools/pen-capture/airbrush_v1_curved_prerequisites.json",
    "tools/pen-capture/verify_airbrush_v1_curved.py",
):
    target = work / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(root / relative, target)
for name in ("scratch", "tmp"):
    (work / name).symlink_to(root / name, target_is_directory=True)
print(work)
PY_REPLAY
```

Coordinate the shared Cargo target, then run the verifier in the printed directory:

```sh
cd /tmp/sdocx-airbrush-curved-REPLACE_WITH_PRINTED_SUFFIX
/home/jding/RustroverProjects/sdocx/.venv/bin/python tools/pen-capture/verify_airbrush_v1_curved.py
```

Native execution requires the original `/home/jding/RustroverProjects/sdocx` workspace: the ignored runner hard-codes this path. Preserve the local runner, runtime helpers, Samsung Notes 4.4.45.37 ARM64 DSOs, Unicorn 2.1.4, ARM64 binutils, libc, nm, c++filt, EGL and GLES files pinned by `airbrush_v1_curved_prerequisites.json`. `libSPenCxx.so` must link to `libc++_shared.so`, and the existing runtime-input symlinks must resolve. Rust dependencies and PyMuPDF 1.28.2 must already be installed; the recorded Python was 3.14.3. The verifier uses the project agent Cargo settings and checks source, prerequisite, native-frame and PDF-raster hashes.

This is a local replay, not a complete acquisition package or machine image. The Mesa vendor driver and some dynamic dependencies, system Python and kernel are not fully pinned. The runner uses synthetic setter arrays and a controlled synchronous queue, not a deserialized user note. Without the local capture environment, the retained receipt still supports the existing Rust regression; native reproduction has not been established on other hosts.

Historical native captures probed tools 0, 2 and 3 under null V1 settings. Three synthetic paths, including
sizes 2.25 and 8.75 and two colors, produce identical native streams and frames
across those tools. The public matrix test compares six streams and SVG rasters;
the candidate matches each native frame exactly. Historical direct-PDF reader
results varied by path. These host controls do not establish device, saved-note,
other-reader or arbitrary-path parity. The optional PDF diagnostics were retired;
[historical findings](../../docs/reverse-engineering/stroke-rendering-findings.md#historical-airbrush-pdf-diagnostics)
summarize their limits.

Two synthetic tool-1 controls have distinct native vectors from tool 2: 27 versus
25 stamps on the zigzag, and 53 each with different contents on the reverse path.
Run `python3 tools/pen-capture/airbrush_v1_tool1_capture.py OUTPUT_DIRECTORY` on
a prepared host to regenerate raw captures. Local runner and pinned prerequisites
above are required; the compact receipt retains native frames and vectors. The
historical opt-in experiment matched these native SVG frames, but its PDF error
regressed for the reverse path in MuPDF. It was not integrated, and does not
establish arbitrary-path or device parity.
