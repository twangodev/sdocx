# Pencil2 bitmap and canvas factory route

Static ARM64 inspection extends the [v250 bitmap gate](../pencil2-worker-v250/README.md). In the pinned `libSPenGraphics.so` (SHA-256 `aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621`), `SPGraphicsFactory::CreateBitmap(IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, TextureMemoryLayout, TextureTargetLayout)` at RVA `0xc04ac` checks for a non-null queue and positive dimensions. On successful allocation it invokes the `SPTextureBitmap` constructor at `0xc0554`, then calls virtual-table byte offset 96 with the supplied format string, and returns the object. Thus this overload offers a direct texture-bitmap construction route without the existing-bitmap prerequisite of the v250 shared-texture overload. Construction does not establish that its textures are ready for drawing.

`SPGraphicsFactory::CreatePenCanvas(IGLMsgQueue*, ISPBitmap*, int)` at `0xc1454` allocates 256 bytes and forwards the queue and bitmap to `SPPenCanvas` when the last argument is nonzero, or `SPPenCanvasRT` when zero. This connects a factory-created bitmap to a factory-created canvas in principle; neither function proves that a real page selects this route or that the resulting canvas is valid through deferred execution.

Reproduce the static observations with the checkout-local cross-objdump and libraries:

```sh
export LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0xc04ac --stop-address=0xc05e0 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0xc1454 --stop-address=0xc14c8 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
```

These disassemblies show PLT calls; actual resolved implementations depend on runtime symbol binding. Reproduction requires the local ARM64 binary and checkout-local cross-objdump and shared libraries at the paths shown; the binary does not independently establish APK provenance. This is static evidence only. No factory call, ownership lifecycle, GL resource readiness, real page output, device fidelity, or improved SVG/PDF output was verified. REA tools were absent in the active session; local binutils provide the stated disassembly.

The constructor path distinguishes two initialization routes. The factory passes `int == 0` as the constructor's second Boolean; a true second Boolean (`int == 0`) selects `SPSubBitmapRT` at call site `0x8f298`, while a false second Boolean (`int != 0`) selects `SPSubBitmap` at call site `0x8f338`. The loop obtains a device capability via `RendererFactory::GetRendererDeviceCaps` and its virtual-table byte offset 16 before entering the sub-bitmap loop. The `SPSubBitmapRT` constructor calls `createTextureRT` at `0x8d974`; that method calls `RendererFactory::CreateTextureObject` at `0x8da64` on its null-object branch. This supplies a specific candidate for a real resource initialization route, but static calls do not establish success, queue execution, GL allocation, or pixel output. The other constructor calls an internal helper at `0x8aa14`; its initialization semantics were not resolved.

```sh
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x8f140 --stop-address=0x8f560 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x8d89c --stop-address=0x8daa0 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
```
