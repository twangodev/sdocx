# WaterColor host raster and composite captures

Run acquisition from the repository root with the existing native runtime:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/capture.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/shader_probe.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/redraw_capture.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/frame_capture.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/prefix_probe.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/ir_capture.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/opacity_probe.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/precision_probe.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/vertex_probe.py
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/varying_probe.py
```

The controllers verify the historical V15 harness digest, native libraries and
host libc/libm. They require its retained source and dependencies under
`tmp/vector-parity-plan/pen-family-audit/watercolor-mesa-paint-v15-density-instance-readback/`
and `tmp/conformance-cleanup/native-capture-a241793/conformance/`. They do not
install software or register REA tools. Native execution uses the shipped ARM64
libraries, the existing synchronous emulator bridge and host Mesa GLES.

`capture.py` repeats the original fifteen public AddPoint tuples 1/4/8/16/32
times and varies the public Draw matrix. It seeds the actual native output
attachment through the public subbitmap framebuffer methods and verifies its
pixels before Draw. A passive read immediately before the native composite
draw verifies that Draw has cleared that same attachment. Final sparse pixels
are relative to transparent black, regardless of the supplied prior. These
controls establish the constructor-default redraw=false layer output.

`redraw_capture.py` uses the public `SetRedrawState(bool)` setter and repeats
Draw on the same RTV1 without changing its AddPoint buffer. It verifies that
false clears the supplied prior, true preserves it, and private masks retain
additive state across draws. Two controls supply explicitly placed pixels from
the retained native Pencil2 frame. `frame_capture.py` uses redraw=true with
non-square 1×/2× target dimensions. `prefix_probe.py` is an explicitly diagnostic
host split into one-instance draws; its final mask/frame hashes must match the
original unsplit control. `ir_capture.py` retains compact compiler excerpts
and ELF blocks identifying the named redraw-state gate. This replaces the
older audit's first-use-clear interpretation. `opacity_probe.py` changes only
the saved color alpha through public SetPenData; its unchanged native frame
now matches the clipped Rust model exactly.

`precision_probe.py` isolates the dense 2× pixel after every stamp.
`vertex_probe.py` configures transform feedback without changing either shader
source and requires all 480 original prefix pixels plus full mask/frame hashes
to remain unchanged. `varying_probe.py` adds a fragment output and records
whether its diagnostic mask still matches the original prefix. That experiment
is rejected as an interpolation oracle: its additional output changes bytes
from stamp 148 onward. Its UV observations must not calibrate the native model.
The lasting Rust regressions reconstruct clipping before varying-plane setup
and compare both prefix traces, every feedback lane and all destination frames.

`shader_probe.py` explicitly replaces the native mask texture after its draw
with all 65,536 red/green byte pairs. It also uploads and verifies destination
pixels immediately before the unchanged native composite draw. These are
diagnostic shader/blend inputs; they do not establish naturally generated mask
density, saved-note state or device behavior.

Rust owns the lasting regressions. Coordinate Cargo with other agents, then:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test -p sdocx --all-features --lib ink::watercolor
```

Receipts retain source inputs, library/GLES identities, verified attachment
boundaries and sparse/compressed pixels. Full per-case capture reports are
temporary and removed after successful acquisition. The explicit public canvas
profile is bounded to its documented source/context contract; normal document
rendering and device parity remain separate work.

## Public profile and paired export checks

Fresh source controls:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/public_capture.py
```

With the required shared Cargo settings above, emit reproducible exports:

```sh
SDOCX_WATERCOLOR_ARTIFACTS="$PWD/scratch/watercolor-public/exports" \
cargo test -p sdocx --all-features --test watercolor_raster
python3 tools/watercolor_host/export_compare.py rust
```

The measurement command needs an existing environment with Pillow, PyMuPDF and
pypdfium2. It checks native pixels through PNG and both SVG-page and retained-scene
PDF paths in MuPDF/PDFium. SVG correctness is asserted by Rust/resvg. The eight WaterColor source controls are native-exact in all measured readers.

The public Rust API is explicit: construct `InkRasterCanvas` with an
`InkRasterContext`, then call `paint(stroke, InkRasterProfile::WaterColorHostRtv1
{ screen_width, screen_height })`. The screen dimensions describe source
sampling; canvas dimensions and `pixels_per_unit` describe the output. The
admitted output scales are 1 and 2. `pixels()` exposes premultiplied top-origin
RGBA8; `to_png`, `rendered_page`, and `to_pdf` export the resolved frame.

Each submission uses a fresh mask and redraw enabled. Source validation and
work exhaustion leave the existing canvas unchanged. The profile is based on
captured host arithmetic and does not alter normal document dispatch or claim
Android device fidelity.

Native inputs and compact JSON receipts stay in conformance. Generated exports
under `scratch/watercolor-public/exports` can be removed after measurement.
