# R8 framebuffer copy control

Run `python3 tools/pencil2-worker-v260/probe.py` after the v219 preparation. The wrapper pins and runs v255, adding a host GL operation immediately after its native Draw: allocate a 192×192 RGBA8 texture, copy the entire currently bound R8 framebuffer with `glCopyTexSubImage2D`, attach the destination to a new framebuffer, and read RGBA bytes. It verifies the original v255 capture and that all 36,864 destination pixels match the source RGBA readback. Both have SHA-256 `bf8ab3ad30331118e6f325e4cee8c2d0bca878cceab13fd096e573443640a051`. On this host the changed pixels remain red, green and blue remain zero, and alpha is 255 throughout; the GL error checks immediately after the copy and after framebuffer setup plus readback each returned zero, and the destination framebuffer is complete. These checks cover those sequences and do not exclude errors consumed by intervening calls. This is an explicit host operation, not a native method invocation. Host GL and Mesa remain unpinned, and the v255 synthetic queue and manual canvas substitution apply.

A separately inspected candidate native copy path is `SPTextureBitmap::CopySubRegionFromBitmapRT` at Graphics RVA `0x90dfc`. For the default concrete SPTextureBitmap, SPSubBitmap and GLES TextureObject vtables, it obtains source sub-bitmap 0, calls its CreateFrameBufferRT and ActivateFrameBufferRT methods, then obtains destination sub-bitmap 0 and its texture, and calls texture slot 88. CreateFrameBufferRT constructs an SPFrameBuffer using the source texture when needed; ActivateFrameBufferRT dispatches to a framebuffer interface. The final concrete framebuffer implementation and binding are not established by this static inspection. Renderer vtable relocation `0x6afb8` resolves that slot to `GLES::TextureObject::CopySubRegionFromFramebuffer` at RVA `0x5f3c8`; it binds the destination texture and calls `glCopyTexSubImage2D` at `0x5f440`. This static chain is conditional on those concrete classes and does not establish that saved-page rendering invokes it. The runtime copy uses the same GL primitive, but does not establish actual page lifecycle, compositing, or device behavior.

The static chain can be inspected with the following local commands. Graphics SHA-256 is `aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621`; Renderer SHA-256 is `f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed`. Cross-binutils and host tools are unpinned.

```sh
sha256sum scratch/apk-analysis-native/arm64-v8a/libSPen{Graphics,Renderer}.so
readelf -sW scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so | c++filt | rg 'vtable for SPen::(SPTextureBitmap|SPSubBitmap)'
readelf -rW scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so | c++filt | rg '^00000000000(cfdd0|cff70|cfdf8|cf188|cf1a0|cf138|cf868|cf880|cf818) '
readelf -sW scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so | c++filt | rg 'vtable for SPen::GLES::TextureObject'
readelf -rW scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so | c++filt | rg '^000000000006afb8 '
export LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x90dfc --stop-address=0x90f64 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x8b53c --stop-address=0x8b5e0 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x8b800 --stop-address=0x8b8b4 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x5f3c8 --stop-address=0x5f444 scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so
```
