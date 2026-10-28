# SimpleBrush 2× host control

The native control requires the local pinned Samsung libraries, Unicorn runtime,
Mesa adapter, and ignored scratch scripts identified in the receipt. It is not a
saved-note or device capture. Run the native wrapper from the repository root:

```sh
python3 tools/simplebrush-scale/capture.py fraction12 /tmp/simplebrush-scale-native.json
```

The wrapper writes a complete host frame next to the requested receipt, under
`captures/`. Compare its frame and stream hashes with the compact receipt. The
other control names are `clipped8` and `reverse20`. Local tools and Mesa versions
can affect pixels.

To generate public full-page SVG and direct PDF controls, use a fresh output
directory and the shared agent Cargo target:

```sh
mkdir -p /tmp/simplebrush-two-export
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/simplebrush-two-export \
cargo test -p sdocx --features pdf,serde --lib two_pixel_native_export_fixture
.venv/bin/python tools/simplebrush-scale/compare.py /tmp/simplebrush-two-export
```

The comparator requires resvg 0.45.1 and MuPDF 1.28.2, validates the retained crop hash,
and checks SVG alpha and direct-PDF RGB over the matched rendered blank page.
Source, paint runner, and SimpleBrush DSO hashes are checked; dynamically loaded
helper and renderer inputs are not all independently pinned. It writes a
transient SVG and PNG to the supplied export directory. It does not establish
arbitrary scale, transforms, device rendering, or saved-note parity.

For the paired tool-0 and tool-3 2× controls, pass `--tool 0` or `--tool 3`
to `capture.py` with `fraction12` and `reverse20`. The wrapper validates each
frame and geometry against the corresponding tool-2 control. Public exports can
be generated with `SDOCX_TOOL=0` or `SDOCX_TOOL=3`; add `SDOCX_REVERSE=1` for
reverse20, and pass `reverse20` as the second argument to `compare.py`. See
`conformance/simplebrush-scale-2x-tools-host.json` for the paired results. The
comparator reuses the tool-2 crop because the paired native geometry and full
frames match tool 2 exactly.

For the tool-2 size-boundary controls, compare public SVG and direct PDF
against the retained native crops without recapturing. For example, for the
reversed size-32 case, from the repository root:

```sh
mkdir -p /tmp/simplebrush-size32-reverse
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/simplebrush-size32-reverse \
PROBE_SIZE=32 SDOCX_REVERSE=1 \
cargo test -p sdocx --features pdf,serde --lib two_pixel_native_export_fixture
.venv/bin/python tools/simplebrush-scale/compare-size.py /tmp/simplebrush-size32-reverse reverse32
```

The case names are `fraction3`, `fraction4`, `fraction32`, `reverse3`, and
`reverse32`. Match `PROBE_SIZE` to the case and set `SDOCX_REVERSE=1` only for
reverse cases. The comparator verifies crop and reconstructed frame hashes and
checks the SVG alpha and matched-paper PDF RGB errors in the receipt. To
recapture a full native frame, run `python3 tools/simplebrush-scale/capture-size.py
reverse32 /tmp/simplebrush-size-native.json` with the local inputs described
above; the wrapper verifies the geometry and frame hashes. Other cases use the
same command with the desired case name.

The 2× setting-alias controls use `--advanced-setting empty` or `--advanced-setting 1`
with `fraction12` and `reverse20`, and each `--tool` value 0, 2, and 3.
The wrapper checks native geometry and complete frames against the null-setting
control. The compact matrix is `conformance/simplebrush-scale-2x-settings-native.json`.
