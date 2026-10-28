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
