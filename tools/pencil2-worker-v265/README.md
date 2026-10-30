# Page-capture factory and object dispatch (static)

In local `libSPenComposer.so` SHA-256 `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f`, `NoteCapturePage::drawPage` calls `makeDrawingBitmap` at `0x32dd98`, then conditionally calls `drawObject` at `0x32dee0` after other page layers, depending on LayerMode. `makeDrawingBitmap` reuses an existing bitmap when both queried dimensions match; otherwise it releases it and calls Graphics `CreateBitmap` at `0x32dfc8`, with stack words 6 and 2, null pixels, booleans 0, integer 1, and caller dimensions. The format string is a separate argument, not the layout. See v264 for the separate synthetic host observation of a 6/2 allocation.

`drawObject` checks a flag and `WPage::GetObjectCount(true)` before conditional base, top and masking layer calls. Base and top each construct an ObjectList and call `getCloneObjectList`; when the respective list count is at least one, base passes the list, the page bitmap and matrix to virtual slot byte offset 16 on the object at page +48 (`0x330054`). For a nonempty list, top allocates another bitmap with stack layouts 6/2 at `0x3301bc`, passes it and its list to the same virtual slot (`0x330200`), creates a canvas on the page bitmap and passes that temporary bitmap and a paint object to virtual slot 312 (`0x3302c4`). `SetXFermode` selects 16 or 17 depending on branches involving PDF and background darkness. These are static call and dataflow observations, not verified concrete virtual targets or blend semantics.

Reproduce using the project-local ARM64 binutils (set `LD_LIBRARY_PATH` to `scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu`):

```sh
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x32dd44 --stop-address=0x32e018 scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x32e700 --stop-address=0x32e844 scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x32ff58 --stop-address=0x3302e0 scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so
```

PLT bindings and virtual targets depend on runtime. This does not establish which capture mode a real saved note uses, that its list contains Pencil2, automatic scheduling, resulting pixels, production SVG/PDF parity, or device behavior. No real page runtime was observed. REA tools were absent in the active session; the above local disassembly is the evidence.
