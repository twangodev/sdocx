# InkPen V4 3× host comparison

`comparison.json` records historical absolute errors for five synthetic 3× host-Mesa
readbacks against the public SVG and direct PDF exporters. The 1 and 3 keys
select the export contour; all outputs are rasterized at 3×. SVG uses resvg
and alpha-channel error; MuPDF and PDFium use RGB error over #fcfcfc. The
fixture includes native stream and raw-frame hashes. These are host controls,
not device or saved-note parity.

Reproduce native frames from the candidate checkout, with its capture inputs
available under `tmp/` and `scratch/`:

```sh
export PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias
INKPEN_CAPTURE_SCALE=3 python3 tools/inkpen-capture/inkpen_v4_scale_capture.py --print-capture > /tmp/v4-3-straight.json
for name in 1 loop phase; do
  INKPEN_CAPTURE_SCALE=3 INKPEN_CAPTURE_BENT="$name" python3 tools/inkpen-capture/inkpen_v4_scale_capture.py --print-capture > "/tmp/v4-3-$name.json"
done
INKPEN_CAPTURE_SCALE=3 python3 tools/inkpen-capture/inkpen_v4_color_scale_capture.py --capture-one --argb 0x80402010 > /tmp/v4-3-color.json
python3 tools/inkpen-v4/pack.py /tmp
```

The packer writes the fixture in the checkout; run it only on a disposable copy
when preserving existing fixtures. The capture scripts use native inputs described by the existing scale receipt;
the loop and phase routes additionally enforce helper hashes. The color-control
adapters do not verify the external runner or caller source hashes, and their
native runtime dependencies are not fully pinned. Retained frame and stream
hashes identify bytes, not the complete acquisition environment.
The `phase` capture was independently rerun with matching stream and RGBA hashes.

The public-export comparator and temporary instrumentation have been retired. `comparison.json` retains historical reader metrics; raster-library versions can change numeric results. Native capture and packing remain available as described above, without promising reproduction of those external-reader measurements.

## Opaque color control

This extends the 3× synthetic host-Mesa controls with opaque ARGB `ffa1b2c3`, using the same straight trajectory and 1920×1080 display context. The native ObjectStroke supplies an empty advanced setting; the public probe explicitly supplies `4;` to select V4. The capture selected V4, but this control does not establish equivalence for other settings or inputs. The native stream hash matches the earlier straight control. The retained native frame and input are in [`opaque/inkpen-v4-3x-native.json`](opaque/inkpen-v4-3x-native.json) and [`opaque/inkpen-v4-3x-native.bin`](opaque/inkpen-v4-3x-native.bin). At 3× rasterization, selecting the scale-3 contour instead of scale 1 changed absolute alpha error in public SVG/resvg from 52,623 to 3,480, and RGB error in direct PDF from 189,304 to 146,957 in MuPDF and 42,741 to 5,442 in PDFium. The PDF comparison composites native premultiplied RGBA over #fcfcfc. Both contours use the same explicit V4 profile; this does not compare against generic fallback. This single synthetic control does not establish device, saved-note, or general color parity.

With the native capture inputs and libraries described above, reproduce and compare the native bytes:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias INKPEN_CAPTURE_SCALE=3 python3 tools/inkpen-capture/inkpen_v4_color_scale_capture.py --capture-one --argb 0xffa1b2c3 > /tmp/v4-opaque-capture.json
python3 tools/inkpen-v4/opaque/pack.py /tmp/v4-opaque-capture.json /tmp/v4-opaque-packed
cmp /tmp/v4-opaque-packed/inkpen-v4-3x-native.json tools/inkpen-v4/opaque/inkpen-v4-3x-native.json
cmp /tmp/v4-opaque-packed/inkpen-v4-3x-native.bin tools/inkpen-v4/opaque/inkpen-v4-3x-native.bin
```

The public comparison figures above are historical measurements; the native input and frame remain available for inspection.
