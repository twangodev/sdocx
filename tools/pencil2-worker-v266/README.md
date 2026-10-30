# Page object drawing dispatch (static)

For the Composer binary identified in [v265](../pencil2-worker-v265/README.md), SHA-256 `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f`, both `NoteCapturePage` constructors call `DrawingFactory::CreateDrawing` and store its result at page +48: `0x32da88`/`0x32da8c` and `0x32dc3c`/`0x32dc40`. In local `libSPenDrawing.so` SHA-256 `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd`, the factory at `0x74934` allocates and constructs `ObjectDrawing`, returning null if `Construct` fails. Its vtable has an ABS64 relocation at `0xc2f80` to `ObjectDrawing::DrawObjectList` (`0x7f098`); with the normal Itanium ABI address point at `0xc2f70`, this is slot byte offset 16, the slot called by the v265 page paths. This resolution assumes the local library bindings and normal constructor/vtable; runtime interposition has not been checked.

`DrawObjectList` creates a canvas and a pen canvas on its supplied bitmap, applies a supplied matrix to both when non-null, and iterates the object list. Its per-object dispatcher checks visibility and dispatches by object type. This connects the page calls to the established ObjectDrawing path, but does not show that an actual saved page supplies Pencil2, which layer it occupies, or what pixels result. The existing v264 synthetic factory and queue captures do not execute this page lifecycle. No native saved-page oracle or production SVG/PDF parity is established; no rendering approximation is promoted here.

Reproduce with the project-local ARM64 binutils, setting `LD_LIBRARY_PATH` to `scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu`:

```sh
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x32d9a0 --stop-address=0x32dc70 scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x74934 --stop-address=0x74988 scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-readelf -rW scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so | grep 00000000000c2f
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x7f098 --stop-address=0x7f6d0 scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so
```

REA tools were unavailable in the active session; these are local static disassembly and relocation observations.

## Stroke selection bridge

The dispatcher's type jump table at Drawing `0x52464` maps `GetType() == 1` to `0x7fd28` (`0x7fae0 + 4 * 0x92`), which tail-calls `ObjectDrawing::drawObjectStroke` with the object and both canvases. This is conditional on the preceding visibility and virtual predicate checks. `drawObjectStroke` at `0x81a20` rejects point counts at most one and null pen names, copies the stroke's `GetPenName()` into a string, rewrites the special name `Eraser` to `com.samsung.android.sdk.pen.pen.preload.Eraser`, then calls `PenManager::GetPenData` with that string at `0x81ad4`; null data or its null pen field follows an error path. It applies stroke properties, including size and color type, to the selected pen and passes the pen canvas, selected pen and stroke type to `setIPenBitmap` at `0x81dd4`. These are static call and dataflow observations, not proof of a particular manager registry or saved document's pen name.

The separately retained [v241 synthetic worker](../pencil2-worker-v241/README.md) observes the list-to-`drawObjectStroke`-to-Pencil2 V14 `RedrawPen` chain using an attached synthetic four-point clone, and [the async-route controls](../pencil2-async-route/README.md) demonstrate why that attachment matters for this adapter. Together these support the conditional bridge from page dispatch to that previously exercised Pencil2 route, but they do not execute `NoteCapturePage` with a saved Pencil2 note. Dynamic bindings, actual object visibility, registry, version, list contents and lifecycle remain unverified.

Additional static reproduction:

```sh
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -s --start-address=0x50996 --stop-address=0x509a0 scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -s --start-address=0x4ffbc --stop-address=0x4ffec scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -s --start-address=0x52464 --stop-address=0x52494 scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x7fa24 --stop-address=0x7fd68 scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x81a20 --stop-address=0x81e50 scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so
```
