# Manager map release control

After v219 preparation, run:

```sh
python3 tools/pencil2-worker-v275/probe.py
PENCIL_NATIVE_RELEASE=1 python3 tools/pencil2-worker-v275/probe.py
```

The probe transforms the SHA-256-pinned v273 script with the v274 RGBA layout and fixture changes, then adds a cleanup control after native message Run. Before cleanup it asserts a one-node manager map containing the canvas. By default, native `PenGLDataManager::ReleaseGLObject` calls `ReleaseCanvas` once with that canvas, whose release body is skipped, and the manager map becomes an empty sentinel, null root and zero size. An empty map does not prove node frees or resource destruction.

In native-release mode, the release body runs and the probe requires the exact `RuntimeError: unhandled native import glDeleteBuffers` from the host adapter. It catches that expected error and exits successfully after the inherited capture checks; native cleanup did not complete. Both modes passed on this host. This synthetic message substitutes the canvas and explicitly initializes the drawable; actual page scheduling and ownership remain unverified.

Static inspection of the pinned PenCommon DSO (SHA-256 `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d`) shows `ReleaseGLObject` iterating map entries, calling ReleaseCanvas, calling the recursive helper at RVA `0x49510`, and resetting the map. The helper invokes operator delete for the nodes; this static path does not prove runtime frees. Inspect with:

```sh
export LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x491bc --stop-address=0x49260 scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x49510 --stop-address=0x49558 scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
```
