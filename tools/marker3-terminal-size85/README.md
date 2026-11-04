# Marker3 terminal size controls

Fresh synthetic Drawing and RTV1 host Mesa captures use size 8.5 at 2× and 4×, with the trajectories, pressures and phases of the retained size-9 expanded controls. The boundary case is a fresh size 9.0001 1× capture. These are explicitly bridged native vectors, not device or saved-note parity.

The boundary caller capture is byte-identical to the retained `marker3-size9-expanded/boundary-caller.json.gz`, which remains the canonical caller capture. The terminal control's separate boundary receipt and frame remain here.

Historically, for size 8.5, baseline/profile SVG, MuPDF and PDFium aggregate L1 were respectively 36285/12487, 159312/141599, 35279/15438 at 2× and 57882/21238, 183682/154513, 64627/32234 at 4×; both modes miss zero native-support pixels. The size 9.0001 case yielded 25926/9580, 51226/39419, 26027/11912 with zero missing pixels. Readers are resvg 0.45.1, MuPDF 1.28.2, pypdfium2 5.14.0 / PDFium 156.0.8076.0. These measurements used an isolated size-gate expansion of revision `9eaa8fc`; they do not establish admission above 9 or general pixel parity. The current tool-1 page gate admits sizes 4–9 subject to other guards.


The experimental exporter, comparator and replay workflow were retired; native
inputs, frames, receipts and capture scripts remain. `SHA256SUMS` checks retained
files only. Retired source hashes remain in Git history.

From the repository root on the prepared host, use a fresh output directory:

```sh
.venv/bin/python tools/marker3-terminal-size85/capture.py /tmp/marker3-terminal-size85-fresh
```

Compare decompressed frames and native vectors with the retained receipt hashes.

Native recapture needs local Samsung DSOs, Unicorn, AArch64 binutils, Mesa/EGL and the ignored helpers described in `tools/marker3-zero/hashes.json`. The full runtime closure is not provided.
