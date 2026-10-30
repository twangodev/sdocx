# Pencil2 planner cost control

`python3 tools/pencil2-planner-bench/run.py` compares commit `504b0654` with
its parent using a temporary release-mode Rust harness and the shared Cargo
target. It extracts the committed sampler and geometry code, then times the
parent sampler against planner plus sampler. The captured fresh-source geometry
uses fixed pressure and tilt; synthetic controls cover dense points, a long
valid stroke and an over-budget stroke. After 10 warmup calls per version, four rounds alternate which version runs
first and report mean time per call. Every call checks the expected record count
or, for the rejection control, specifically `WorkLimit`. The harness pins Cargo
dependencies with its lockfile, uses release optimization level 3 with debug info
disabled and empty Rust flags, and prints the active Rust compiler version. Results
depend on the machine and toolchain; CPU frequency and scheduling are not controlled. The benchmark excludes source conversion and
raster painting; it measures neither end-to-end rendering nor memory usage.
