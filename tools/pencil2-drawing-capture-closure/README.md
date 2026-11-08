# Pencil2 Drawing capture source archive

These are byte-preserved sources used by the synthetic Drawing probes in
`tools/pencil2-public-replay`, including the V12 candidate named in their
receipts. `restore.py` checks their historical temporary paths without writing files. The V10
baseline is compressed losslessly and verified against the SHA-256 in V12
before restoration. Its role is V12's historical input and route join, not a
new capture. The V10 and V12 scripts, plan, source receipt, and three adapters
retain their original bytes. The pinned `fountain_native.py` and `elf_plt.py`
are taken from the tracked Montblanc helpers after hash verification.

The retained V12 candidate expects its original paths and separately pins native
libraries, Unicorn, toolchain, and host components under `scratch/` and system
paths. Those inputs are not included here. The Drawing scripts also depend on
other project-local scripts and runtime inputs; restoring these sources alone
does not establish reproducibility. No new native capture was run as part of
this archival step. See the Drawing receipts for the recorded hash identities
and synthetic queue/host Mesa limitations; they do not establish device or
saved-note parity.

Run `python3 tools/pencil2-drawing-capture-closure/restore.py` for a read-only
check. Missing files and files with different hashes cause a failure. This archive
does not restore historical paths or provide a complete capture environment.

## Isolated native Drawing reproduction

`run.py` reconstructs the historical path layout in a disposable `/tmp` tree,
checks the archived source hashes and the Drawing adapter and GLES helper,
and uses caller-supplied native libraries and runtime by symlink. It leaves
those inputs untouched. The V12 preflight checks its pinned native libraries,
Unicorn, binutils, Python and host libm; the Drawing adapter also checks the
Renderer DSO. The adapter's five embedded checkout paths are rewritten only
in its temporary copy. No binary or runtime package is bundled here.

```sh
python3 tools/pencil2-drawing-capture-closure/run.py \
  --native-dir /absolute/path/to/arm64-v8a \
  --runtime-dir /absolute/path/to/apk-analysis-runtime \
  --receipt /tmp/pencil2-reproduction.json
```

The runtime directory must contain the pinned `python/unicorn` and
`binutils-aarch64` layout named by V12. The output path must not already exist.
The runner compares geometry and raw 192x192 framebuffer SHA-256 with
the SHA-256 pinned `tools/pencil2-public-replay/drawing-native-target-receipt.json`; its exit
status is nonzero on mismatch. It deletes temporary captures on exit. The
retained `native-reproduction-receipt.json` records a successful local run,
with the historical inputs currently under `scratch/`. Capture JSON hashes may
vary with the temporary paths. Host EGL/GLES and Mesa are not fully pinned;
pixel equality is evidence for this host run, not a portability guarantee.
The synthetic input, callback queue intervention, host adapters, identity draw
matrix and transparent background do not establish device or saved-note parity.

## First-point recapture

The archived `first_point_*` sources retain the translated Drawing adapter,
independent-object adapter and patch at the hashes recorded in the original
first-point receipt. The control constructs the same historical layout in a
new disposable tree and checks the archive hashes; the ARM64 libraries and
runtime remain external inputs:

```sh
python3 tools/pencil2-origin-offset/first-point-recapture.py \
  --native-dir /absolute/path/to/arm64-v8a \
  --runtime-dir /absolute/path/to/apk-analysis-runtime \
  --receipt /tmp/first-point-recapture.json
```

The output path must not exist. Assertions must be enabled. This is a finite
synthetic queue intervention using native ARM64 emulation and host Mesa; it
does not establish device or saved-note parity. Mesa, libm and transitive host
runtime dependencies are not fully pinned. The original verified receipt is
preserved alongside the new closure reproduction receipt.

Some local Drawing and lifecycle scripts also expect the independent adapter
and patch at their historical workspace paths. Their exact bytes are already in
this archive; restore those paths before running those local captures:

```sh
mkdir -p tools/pencil2-independent-drawing tools/pencil2-mixed-drawing
cp -n tools/pencil2-drawing-capture-closure/sources/first_point_independent_capture.py tools/pencil2-independent-drawing/capture.py
cp -n tools/pencil2-drawing-capture-closure/sources/first_point_capture.patch tools/pencil2-mixed-drawing/capture.patch
sha256sum -c <<'HASHES'
974a26d6ccea578fc642ae4c8173f4d5bba7cba5ab5c9f11510fdb4622f300d7  tools/pencil2-independent-drawing/capture.py
b9342438df1b8099caf4185a0dc6ea692d7726e48014e921866ef6c3b4389a83  tools/pencil2-mixed-drawing/capture.patch
HASHES
```

`cp -n` leaves an existing file untouched; the hash check detects differences.
Other native and host prerequisites remain necessary for those captures.
