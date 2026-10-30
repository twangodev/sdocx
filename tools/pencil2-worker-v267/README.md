# Pen lookup and fallback (static)

In local `libSPenPenCommon.so`, SHA-256 `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d`, `PenManager::Construct` at `0x4cca0` creates an empty per-manager vector and registers a `PenGLDataManager` with `PenManagerST`. `PenManager::GetPenData` at `0x4cefc` compares the requested name against its cached entries. On a miss it calls `PenManagerST::GetPen` with that name and, on success, caches a new entry and calls pen virtual slot byte offset 360 with the manager's GL data manager.

If that requested-name call returns null, the code first searches the cache for `com.samsung.android.sdk.pen.pen.preload.InkPen`; if absent, it requests `com.samsung.android.sdk.pen.pen.preload.DefaultPen` and creates an entry for that fallback. Thus a non-null `PenData` alone cannot establish that a requested Pencil2 pen was selected. The code does not check the DefaultPen result for null before eventually dereferencing its pen pointer for the virtual call. This describes static branches; runtime bindings and actual error behavior are not verified.

`PenManagerST::GetPen` at `0x4dc2c` calls `createPen` with the name. At `0x4deec`, `createPen` compares the name against a runtime-maintained entry vector, loads the matching entry's library, resolves `createPenInst` with `dlsym`, and calls the resolved function. This does not establish the contents or initialization of the vector in an actual note session, or that the Pencil2 library was selected.

Reproduce with project-local ARM64 binutils:

```sh
export LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x4cca0 --stop-address=0x4ce14 scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x4cefc --stop-address=0x4d238 scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x4dc2c --stop-address=0x4deec scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x4deec --stop-address=0x4e0e0 scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
```

REA tools were unavailable in this session; this is local disassembly evidence.

No original saved Pencil2 note or native saved-page run was identified in this investigation. Prior retained source controls use transformed Fountain inputs or synthetic strokes. The registry contents, page lifecycle, native saved-page pixels, and source-driven SVG/PDF parity therefore remain unverified.
