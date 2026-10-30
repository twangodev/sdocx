# Pencil2 render-message bitmap gate

Static inspection of the local ARM64 artifacts identifies a concrete bitmap
requirement in `PenGLRenderMsg::Run`. The [v245 check](../pencil2-worker-v245/check.py)
ties RVA `0x4676c` to the `PenGLRenderMsg` vtable Run entry; objdump's
nearest-symbol label names a preceding unrelated exported symbol. At common RVA `0x467b0`, canvas virtual
slot 192 returns a bitmap; Run calls its slot 168 and exits if the result is
null. With the default `SPPenCanvas` vtable (address point `0xd0cc8` in
Graphics), slot 192 is `GetBitmap` at `0xaef08`, which reads the impl at
canvas offset 32 and its bitmap at impl offset 8. Slots 232 and 240 are
`EnableClipRT` and `DisableClipRT`, used around each sub-bitmap draw. Canvas
slots 200 and 208 resolve to `Ref` and `Unref`; these delegate to `RefCount`
at canvas offset 8. This alone does not establish the message's ownership
lifecycle.

For a default `SPScreenBitmap` (address point `0xce870`), slot 168 resolves
to `SPBitmap::GetTextureBitmap` at `0x861cc`, which returns null. Thus this
particular bitmap fails Run's gate before Update. For a default
`SPTextureBitmap` (address point `0xcfd28`), slot 168 resolves to
`SPTextureBitmap::GetTextureBitmap` at `0x92020`, returning the secondary
interface at object offset `0xb0`. Its secondary interface slots 64 and 72
resolve through thunks to `GetSubBitmap(int)` and `GetSubBitmapCount()`;
Run uses these at common RVAs `0x46844` and `0x46810`. The Graphics factory
overload at `0xc0740` itself checks input bitmap slot 168 and, when nonnull
and allocation succeeds, constructs `SPSharedTextureBitmap`; this is an available path, not proof of
the class selected by a page or the ownership and readiness of its textures.

The following commands inspect the evidence (cross-objdump and its libraries
are checkout-local and unpinned):

```sh
sha256sum scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
readelf -sW scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so | c++filt | rg 'vtable for SPen::(SPPenCanvas|SPScreenBitmap|SPTextureBitmap)'
readelf -rW scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so | c++filt | rg '^00000000000(d0d88|d0d90|d0d98|d0db0|d0db8|ce918|cfdd0|cff70|cff78) '
export LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x4676c --stop-address=0x469a0 scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x861cc --stop-address=0x861d4 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x92020 --stop-address=0x92028 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0xc0740 --stop-address=0xc0828 scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so
```

The Graphics SHA-256 is `aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621`;
the PenCommon SHA-256 is `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d`.
These local binaries do not independently establish APK provenance. The vtable
resolutions assume these concrete classes and no interposition. This static
inspection does not observe real canvas construction, page selection, queue
scheduling, resource lifetime, or device output. The earlier synthetic
[message capture](../pencil2-worker-v249/README.md) replaces its incomplete
canvas and does not validate this native hierarchy.
