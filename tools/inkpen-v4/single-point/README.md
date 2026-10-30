# InkPen V4 Drawing single-point control

A synthetic native ObjectStroke with one point, curve enabled, tool 1, size 10,
black color, and empty advanced setting was submitted to `drawObjectStroke` at
1920×1080. Native getters confirmed the input count and empty setting. Drawing
returned 0 with no loader call, factory-created pen, or observed task-queue registration or
dispatch during Drawing. Thus this selected caller did not reach a V4 drawable; this is not a pixel capture
or evidence for every saved object or device.

A four-point control using the same unmodified harness and empty setting
returned 1, selected `InkPenStrokeDrawableGLV4`, and emitted 26 RTV1 callbacks.
The harness executes native Model and Drawing with synthetic data and host
stubs for context and task-queue registration. The observed library and helper hashes are recorded in [receipt.json](receipt.json). The compressed caller source is retained in `caller.py.gz`; its uncompressed SHA-256 is checked by the wrapper. The three imported Python helpers are also retained compressed and hash checked. The wrapper reconstructs the script layout in a temporary directory and removes it on exit. Native libraries, Unicorn and binutils remain external prerequisites under `scratch/`; no scripts under `tmp/` are required.
The one-point wrapper changes the synthetic arrays in memory and stops directly
after the native Drawing call; it does not force a queue or invoke redraw.

With the external native libraries and runtime installed:

```sh
export PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin
export LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu
python3 tools/inkpen-v4/single-point/capture.py --print-capture --advanced-setting empty
python3 tools/inkpen-v4/single-point/capture.py --positive
```

The first output is JSON, including the Drawing result, queue events, native
getter snapshot and source arrays; the wrapper asserts all recorded fields. The positive probe verifies its return, V4 drawable, callback count, stream hash,
and the recorded library and helper hashes. The extracted caller and helpers retain the original filenames and bytes for the original harness hash checks.
