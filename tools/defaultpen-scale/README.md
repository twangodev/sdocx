# DefaultPen output-scale controls

Twenty-nine synthetic native Model/Drawing and host-Mesa controls retain inputs,
VBOs, projection matrices, frames and hashes. The harness supplies synthetic
setters, overrides RT paint color and bridges the Drawing-selected vector to
native update/draw. Framebuffer resizing changes projection; it is not recorded
zoom. These controls do not establish serialized-note or Android-device parity.

Comparisons use opaque #fcfcfc paper, public SVG and direct document PDF. The
0.2-output-pixel edge adjustment is empirical; resvg and PDF readers sample
differently. Reduced RGB L1 error is not pixel identity. Other transforms,
scales, readers, devices and notes remain unverified. The controls below do not
change production behavior.

Native recapture requires a prepared host with project-local pinned Samsung
DSOs, Unicorn, AArch64 binutils, Mesa/EGL and ignored helpers referenced by the
scripts. Wrappers hard-code the original project root and temporary inputs;
there is no clean-checkout acquisition recipe or fully pinned dependency closure.
Use fresh output directories and verify captures before interpreting them:

```sh
.venv/bin/python tools/defaultpen-scale/capture.py /tmp/defaultpen-capture
.venv/bin/python tools/defaultpen-scale/verify.py /tmp/defaultpen-capture
```

```sh
.venv/bin/python tools/defaultpen-scale/tool0_capture.py --tool 0 /tmp/defaultpen-tool0 --case tool0
.venv/bin/python tools/defaultpen-scale/tool0_capture.py --tool 1 /tmp/defaultpen-tool1-control --case tool0
python3 tools/defaultpen-scale/verify_tool0.py --tool0 /tmp/defaultpen-tool0 --tool1 /tmp/defaultpen-tool1-control
```

For the base `capture.py` and `verify.py` pair, add `--case tool2red` to
select one family; verification without `--case` requires all cases. The
separate `verify_tool0.py` command above has no `--case` option. The historical export comparison
covered the original 28 cases with PyMuPDF 1.28.2, pypdfium2 5.14.0 and PDFium
156.0.8076.0. The separate historical `zero-half` comparison used PyMuPDF 1.28.2,
pypdfium2 5.9.0 and PDFium 150.0.7869.0: RGB L1 falls from 1,264 to 699 in
SVG, 2,386 to 1,599 in MuPDF and 1,527 to 699 in PDFium.

The separate size-3, four-point tool-0/tool-1 control has eight/six Drawing
instances; same-scale native RGBA hashes differ at 1×, 2× and 4×. Its
[native receipt](../../conformance/defaultpen-tool0-scale-native.json) retains
compressed readbacks, streams, library identities and bridge details. Stream
hashes match Drawing sources and GL errors are zero. Run
`python3 tools/defaultpen-scale/verify_tool0.py` without paths to check retained
bytes; its capture wrapper pins five local sources and checks substitutions.

Historical public-export comparisons showed reduced error for both tools at all three scales in SVG, MuPDF and PDFium.

## DefaultPen tool-0 wider path control

A size-8 tool-0 path uses (10.25, 9.5), (16.5, 23), (27.75, 8.25),
(45.5, 23.5), with pressure 0.25/0.5/0.75/1. Its 16 native instances and
frames are retained at 1×, 2× and 4×. The existing profile reduces RGB L1
in SVG and both PDF readers at every scale.

Run the verifier without a path to check retained bytes. The historical export
comparator deletes generated exports; its full dependency closure is not pinned.

```sh
.venv/bin/python tools/defaultpen-scale/tool0_diverse_capture.py --tool 0 /tmp/defaultpen-tool0-diverse --case tool0
python3 tools/defaultpen-scale/verify_tool0_diverse.py /tmp/defaultpen-tool0-diverse
```


## Tool-0 tilt and orientation control

Replacing the wider control's zero channels with tilt 0.1/0.4/0.8/1.2 and
orientation 0.2/1.1/2.4/5.3 leaves native streams and RGBA frames byte-identical
at 1×, 2× and 4×. Public SVG and PDF errors also match the wider control
(MuPDF 1.28.2, PDFium 150.0.7869.0). This is not general channel invariance.
The [receipt](tool0-channels-receipt.json) records input bits, hashes and exports.

```sh
.venv/bin/python tools/defaultpen-scale/tool0_channels_capture.py --tool 0 /tmp/defaultpen-tool0-channels --case tool0
python3 tools/defaultpen-scale/verify_tool0_channels.py /tmp/defaultpen-tool0-channels
```

The verifier checks inputs, DSOs, streams, frames, GL errors, native capture script pins and
adapted fixture hash without Cargo.


## Tool-0 reversal and alternating pressure

A size-8 path reverses twice through (8, 16), (42, 16), (12, 18), (48, 20),
with pressure 1/0.15/1/0.15. Native Drawing emits 21 instances at each of
1×, 2× and 4×. The [native fixture](tool0-reversal-native.json) retains streams,
frames, projection and observed native color (0.2, 0.4, 0.6, 1.0), used by the
historical comparison. The profile reduces SVG and both PDF-reader errors at every scale.

```sh
.venv/bin/python tools/defaultpen-scale/tool0_reversal_capture.py --tool 0 /tmp/defaultpen-reversal --case tool0
python3 tools/defaultpen-scale/retain_tool0_reversal.py /tmp/defaultpen-reversal > /tmp/defaultpen-reversal-native.json
cmp /tmp/defaultpen-reversal-native.json tools/defaultpen-scale/tool0-reversal-native.json
```

## Tool-0 uniform-pressure pair

The wider size-8, four-point path produces identical 16-instance streams and
host frames with all-zero, all-one and alternating pressure at 1×, 2× and 4×.
The [native receipt](tool0-pressure-receipt.json) retains both uniform inputs
and output hashes; the diverse fixture supplies shared streams and frames.
This establishes pressure insensitivity only for these inputs and this tool/path.

Public exports were tested only at full pressure: geometry matches the stream,
and SVG and both PDF-reader errors equal the wider alternating-pressure control,
improving at every scale.
Run the native verifier without paths to check retained data.

```sh
.venv/bin/python tools/defaultpen-scale/tool0_pressure_capture.py --pressure zero --tool 0 /tmp/defaultpen-zero --case tool0
.venv/bin/python tools/defaultpen-scale/tool0_pressure_capture.py --pressure full --tool 0 /tmp/defaultpen-full --case tool0
python3 tools/defaultpen-scale/verify_tool0_pressure.py --zero /tmp/defaultpen-zero --full /tmp/defaultpen-full
```

## Tool-0 narrow fractional-coordinate control

A size-1 path uses (10.125, 12.375), (13.625, 12.875), (18.375, 14.125),
(24.875, 15.375). Native ObjectStroke GetPoint preserves all fractional inputs;
MotionEvent GetX/GetY returns the final 24.875/15.375, recorded in the
[getter receipt](tool0-subpixel-getters.json). The
[native fixture](tool0-subpixel-native.json) contains eight instances and frames.
The profile reduces SVG and both PDF-reader errors at 1×, 2× and 4×.

The integer-coordinate variant changes native stream and framebuffer hashes
at every scale and also improves all three export comparisons. Its [native fixture](tool0-integer-native.json) retains the captured evidence.
Reproduce fractional/getter and integer controls, respectively:

```sh
.venv/bin/python tools/defaultpen-scale/tool0_subpixel_capture.py --tool 0 /tmp/defaultpen-subpixel --case tool0
python3 tools/defaultpen-scale/retain_tool0_subpixel.py /tmp/defaultpen-subpixel > /tmp/defaultpen-subpixel.json
cmp /tmp/defaultpen-subpixel.json tools/defaultpen-scale/tool0-subpixel-native.json
python3 tools/defaultpen-scale/capture_subpixel_getters.py /tmp/defaultpen-subpixel > /tmp/defaultpen-getters.json
cmp /tmp/defaultpen-getters.json tools/defaultpen-scale/tool0-subpixel-getters.json
```

```sh
.venv/bin/python tools/defaultpen-scale/tool0_integer_capture.py --tool 0 /tmp/defaultpen-integer --case tool0
python3 tools/defaultpen-scale/retain_tool0_integer.py /tmp/defaultpen-integer > /tmp/defaultpen-integer.json
cmp /tmp/defaultpen-integer.json tools/defaultpen-scale/tool0-integer-native.json
```

## Tool-0 repeated points

A size-1 input repeats both endpoints: (10.25, 12.5), (10.25, 12.5),
(24.5, 18.25), (24.5, 18.25), with pressure 0.25/0.5/0.75/1. The
[native fixture](tool0-repeat-native.json) retains streams and frames.
ObjectStroke GetPoint returns all four repeated coordinates; MotionEvent
GetX/GetY returns the last coordinate in observed Drawing and saved-redraw
calls. The [getter receipt](tool0-repeat-getters.json) retains raw values and
hashes. Reconstructed export geometry matches the stream, and the profile
reduces SVG and both PDF-reader errors at 1×, 2× and 4×. These are historical export comparisons.

```sh
.venv/bin/python tools/defaultpen-scale/tool0_repeat_capture.py --tool 0 /tmp/defaultpen-repeat --case tool0
python3 tools/defaultpen-scale/retain_tool0_repeat.py /tmp/defaultpen-repeat > /tmp/defaultpen-repeat-native.json
cmp /tmp/defaultpen-repeat-native.json tools/defaultpen-scale/tool0-repeat-native.json
python3 tools/defaultpen-scale/capture_subpixel_getters.py /tmp/defaultpen-repeat > /tmp/defaultpen-repeat-getters.json
cmp /tmp/defaultpen-repeat-getters.json tools/defaultpen-scale/tool0-repeat-getters.json
```

## Tool-0 crossing control

A size-3 path crosses through (10.5, 8.25), (42.25, 23.5), (10.5, 23.5),
(42.25, 8.25), with pressure 0.25/0.5/0.75/1. Native callback streams match
the Rust fixture and GL errors are zero. The profile reduces SVG and both
PDF-reader errors at 1×, 2× and 4×. The
[native fixture](tool0-crossing-native.json) and
[export receipt](tool0-crossing-export.json) retain measurements, hashes and
reader versions; comparison checks Drawing-callback/bridged-stream hash equality
and pins capture/comparator sources.

This export comparator uses clean source from revision
`8ef491537f96154099e7c0c4066c25bba0f4aa0a`, locked Cargo dependencies and exact
Rust test inputs. The receipt records revision and tool dependency hashes.
Native recapture still needs the prepared host and ignored helpers listed in
`hashes.json`; the capture rejects existing output directories.

```sh
.venv/bin/python tools/defaultpen-scale/tool0_crossing_capture.py --tool 0 /tmp/defaultpen-crossing-fresh --case tool0
python3 tools/defaultpen-scale/retain_tool0_crossing.py /tmp/defaultpen-crossing-fresh > /tmp/defaultpen-crossing-native.json
cmp /tmp/defaultpen-crossing-native.json tools/defaultpen-scale/tool0-crossing-native.json
PYTHONPATH=/path/to/pypdfium2 .venv/bin/python tools/defaultpen-scale/compare_tool0_crossing.py --verify
```
