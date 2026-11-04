# Marker3 terminal size controls

Fresh synthetic Drawing and RTV1 host Mesa captures use size 8.5 at 2× and 4×, with the trajectories, pressures and phases of the retained size-9 expanded controls. The boundary case is a fresh size 9.0001 1× capture. These are explicitly bridged native vectors, not device or saved-note parity.

The boundary caller capture is byte-identical to the retained `marker3-size9-expanded/boundary-caller.json.gz`, which the replay shares. The terminal control's separate boundary receipt and frame remain here.

`PYTHON=.venv/bin/python PYTHONPATH=/tmp/sdocx-marker3-v133-readers bash tools/marker3-terminal-size85/replay.sh` archives commit 9eaa8fc, temporarily expands its size guard, verifies input float bits, compares both native caller vectors bit for bit, and rasterizes actual SVG and direct PDF. Requires the offline lockfiles and readers described in `marker3-size9-expanded`. `capture.py OUTPUT` regenerates native captures using the pinned local host inputs.

For size 8.5, baseline/profile SVG, MuPDF and PDFium aggregate L1 are respectively 36285/12487, 159312/141599, 35279/15438 at 2× and 57882/21238, 183682/154513, 64627/32234 at 4×; both modes miss zero native-support pixels. The size 9.0001 case yields 25926/9580, 51226/39419, 26027/11912 with zero missing pixels. Readers are resvg 0.45.1, MuPDF 1.28.2, pypdfium2 5.14.0 / PDFium 156.0.8076.0. The geometry fix removes the previous counterexample; remaining reader errors and limited capture coverage leave wider production admission unproven.
