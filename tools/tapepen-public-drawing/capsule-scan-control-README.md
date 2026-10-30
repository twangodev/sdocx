# TapePen source capsule scan control

`capsule-scan-control.py` is an experimental source-geometry rasterizer for the locally extracted pattern 1 SVG. It parses the SVG's axis-aligned `M...V` and `M...L` strokes, treats each as a round-ended capsule, computes analytic horizontal coverage at fixed vertical samples, and composites strokes in document order over the SVG background. It does not use native pixels as rendering input. It compares both generated textures with captured uploads from the existing 256 and 512 display controls; the direct native rasterizer separately matched those upload hashes.

Run from the repository root with `python3 tools/tapepen-public-drawing/capsule-scan-control.py`. It requires the locally extracted `scratch/apk-output/samsung-notes-4.4.45.37/res/raw/tape_pattern_01.svg` and the retained compressed capture logs. Results below are `(dimension, vertical samples, RGBA L1, maximum channel error, differing channels)`:

```text
64 5 31744 23 3040
64 10 42880 32 3840
64 20 36992 27 3840
128 5 48960 32 3840
128 10 70336 32 6144
128 20 69440 32 6144
```

These are geometry approximations, not a NanoSVG implementation. Five samples marginally improves the prior resvg 128 L1 of 58,560 but changes more channels; increasing the sample count does not converge to the native result. The experiment does not implement NanoSVG's polygon flattening, fixed-point coverage and blending, and does not establish changed-input behavior. It is not installed in production rendering or a public export profile.

The pinned AArch64 library's `nsvgRasterize` disassembly provides a more specific scan model. At RVAs `0x2f3b0`–`0x2f3c8` and `0x2f6c4`–`0x2f6d4`, each output row runs five subscans at `(5*y + k + 0.5)` in the internally vertically scaled coordinate system. Edge x values use 10 fractional bits (`0x2f534`–`0x2f5a4`); the span helper at `0x340a0`–`0x34174` accumulates full-pixel contributions of 51 and partial contributions using integer multiply and shift by 10. The solid-color path uses 8-bit coverage and integer channel blending (`0x2f77c`–`0x2f810`). These are static observations of the pinned library, not a claim that the capsule edges themselves match native flattened paths.

`python3 tools/tapepen-public-drawing/capsule-fixed-scan-control.py` substitutes this fixed-point span accumulation and integer color blending into the analytic capsule experiment. The 64 texture gives L1 31,680, maximum 22, 3,040 differing channels; 128 gives L1 47,552, maximum 31, 4,352 differing channels. This narrows the numerical model but still leaves substantial error; native edge generation and its exact rounding remain unresolved. The script uses Python's rounding and analytic circular caps, so it does not reproduce all native operations.

A subsequent source comparison tested the public upstream NanoSVG C parser and rasterizer against the same native oracle. The upstream source was retrieved from `https://raw.githubusercontent.com/memononen/nanosvg/master/src/nanosvg.h` and `nanosvgrast.h`; its zlib-style notice is in each upstream header. The retrieved headers had SHA-256 `f1fb86526672f1844af1a30a20007425deab21a195243df5dc9fe9abf23a90e9` and `2b146cc47d947f55a2ecf0660a424fdc22bd843b22154ba898f2c6bf7b0ee0ae`, respectively. The precise upstream commit was not established. Compiled with `cc -O2` on this x86 host, `nanosvg-upstream-probe.c` matched both original capture textures byte for byte. This is source comparison evidence, not a Rust or WASM implementation.

`nanosvg-changed-source.py --run --shape64 --size12` additionally calls the same native rasterizer with the pinned SVG after replacing all stroke colors `#3F5480` with `#AD2244`, and separately after replacing all `stroke-width="1.7"` with `stroke-width="2.3"`. It uses a private temporary directory for the modified SVGs and native RGBA bytes, verifies the 136 replacements in each case, and checks source and output hashes against `nanosvg-source-receipt.json`. These source-derived changes exercise both color blending and stroke edge generation. The probe uses the native capture's pinned glibc math and qsort adapters and inherits its limitations. The upstream C executable matched all four native results byte for byte:

| Change | Size | Native RGBA SHA-256 |
| --- | --- | --- |
| color | 64 | `7243f4e9fb60d921b7dc94dac72a197bf5632582e23a7e0056e59baf24bdc5d5` |
| color | 128 | `66b7258d7acb3abd69c1919f4b5739ed574f102badfe124f61340b6445647b37` |
| width | 64 | `9f19ac6ab3702de697d059ab6da77b91edf233f62584dfb2e6c23a5a56ef771e` |
| width | 128 | `76d5e8064e7ab1b93f002d54095f514dc58ed06e180ddb580861a1f131beb6bf` |

The probe requires the extracted APK and capture dependencies documented in `native-pattern-source-README.md`. To reproduce the C comparison, obtain the two upstream headers from the URLs above and verify their exact hashes before using them. Upstream `master` may change; these hashes identify the tested bytes, and no commit identity has been established. Put the verified headers in a private directory and run from the repository root (replace `/path/to/verified-headers` with that directory):

```sh
header_dir=/path/to/verified-headers
printf '%s  %s\n' \
  f1fb86526672f1844af1a30a20007425deab21a195243df5dc9fe9abf23a90e9 "$header_dir/nanosvg.h" \
  2b146cc47d947f55a2ecf0660a424fdc22bd843b22154ba898f2c6bf7b0ee0ae "$header_dir/nanosvgrast.h" | sha256sum -c - || exit 1
probe_dir=$(mktemp -d /tmp/sdocx-nanosvg-probe.XXXXXX)
trap 'rm -rf "$probe_dir"' EXIT
cc -O2 -I "$header_dir" tools/tapepen-public-drawing/nanosvg-upstream-probe.c -lm -o "$probe_dir/nsvg-probe"
NSVG_PROBE="$probe_dir/nsvg-probe" UV_CACHE_DIR=/tmp/sdocx-uv-cache uv run --offline --with unicorn==2.1.4 python tools/tapepen-public-drawing/nanosvg-changed-source.py --run --shape64 --size12
```

The script checks each mutated source and native output against the receipt and compares the C executable's output byte for byte. It requires the exact external header contents; the upstream license notice must be retained if those headers or derived source are redistributed. The source comparison supports implementing NanoSVG's path flattening, round-cap tessellation, fixed-point active edges, winding, and integer blending in portable Rust; it does not establish parity for arbitrary SVGs, other native builds, or Android math.
