# Marker3 size-9 historical counterexamples

Three synthetic tool-1 Model/Drawing captures extend `../marker3-size9-exploratory`. `phase2` and `phase4` use size 9 with different trajectories, pressure and coordinate phase, and native Draw matrices at 2× and 4× respectively. `boundary` uses size 9.0001 at 1×. The capture uses the same retained native Drawing vector bridged into its selected RTV1, with Mesa llvmpipe, as documented by the prior diagnostic; it does not establish Android device or saved-note parity. The 256×256 native frame bounding boxes are respectively [34,41,115,88], [54,57,192,135], and [17,25,72,44], all inside the compared 256×256, 256×256 and 128×128 regions. No native support is clipped by those regions.

At revision `4361933850036a3c2e5a333814f04b499f47fe6a`, the first input yielded 12 candidate geometry points versus 37 native Drawing points. That historical counterexample was subsequently corrected; the maintained terminal geometry test checks the retained vectors. Aggregate L1 baseline/profile historical values for SVG, MuPDF and PDFium were 215951/211257, 279780/275490, 154622/149290 at 2×; 582380/577915, 533738/527887, 416842/409359 at 4×; and 59547/51176, 73736/67455, 45898/38384 at size 9.0001. Baseline and candidate both missed 694, 1871, and 155 native-support pixels respectively. Smaller aggregate error therefore does not establish parity or justify admission.


The maintained `terminal_segment_matches_expanded_native_vectors` Rust test
checks Drawing/saved-redraw geometry for the terminal and expanded controls,
including rejection of the 9.0001 page case. The current tool-1 page size range
is 4–9 subject to its other guards. Historical reader metrics above are not
current external-reader assertions.

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test -p sdocx --lib --features render,serde terminal_segment_matches_expanded_native_vectors
```

The experimental exporter, comparator and replay workflow were retired; native
inputs, frames, receipts and capture scripts remain. `SHA256SUMS` checks retained
files only. Retired source hashes remain in Git history.

From the repository root on the prepared host, use a fresh output directory:

```sh
.venv/bin/python tools/marker3-size9-expanded/capture.py /tmp/marker3-size9-expanded-fresh
```

Compare decompressed frames and native vectors with the retained receipt hashes.

Native recapture needs local Samsung DSOs, Unicorn, AArch64 binutils, Mesa/EGL and the ignored helpers described in `tools/marker3-zero/hashes.json`. The full runtime closure is not provided.
