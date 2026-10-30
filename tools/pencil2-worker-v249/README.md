# Pencil2 deferred message before canvas replacement

Run `python3 tools/pencil2-worker-v249/probe.py` to verify the receipt, or add `--write-receipt` to refresh it. This extends v248's synthetic EventLoop and explicit renderer initialization. Before replacing the deferred message's canvas, it inspects the original pointer, virtual table and message matrix.

The original pointer is the harness's synthetic pen canvas, not its synthetic drawing canvas. Its virtual slots at offsets 232 and 240 are null, so it does not provide the bitmap hierarchy needed for this message's Run path. The native-created message already stores the 64-byte Matrix4 `[1,0,0,0, 0,-1,0,0, 0,0,1,0, 6,183,0,1]` at offset 72 before replacement. This is the same matrix subsequently observed at Draw; the replacement changes the canvas pointer, not the matrix. The Matrix3 setter in this run is itself supplied by the harness.

The remaining Run uses the fabricated canvas and bitmap hierarchy described by v248 and produces its same host Mesa readback. These observations do not establish normal canvas construction, scheduling, renderer initialization, bitmap ownership, saved-page behavior or device fidelity. Host runtime dependencies are not fully pinned.
