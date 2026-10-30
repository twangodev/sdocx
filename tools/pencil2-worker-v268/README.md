# Packaged registry and controlled selection

For the `libSPenPenCommon.so` identified in `../pencil2-worker-v267/README.md`, `PenManagerST::Construct` calls `buildList` at `0x4d99c`. The latter reads pairs of pointers from `0x78968`, constructs two strings per pair, and appends entries to its vector, with 45 iterations on the successful allocation path (`0x4dbac`). The table's zero-based entry 24 has relative relocations at `0x78ae8` and `0x78af0` to the strings `com.samsung.android.sdk.pen.pen.preload.Pencil2` and `Pencil2`. The locally extracted package contains `libSPenPencil2.so`. `loadlibrary` constructs a library name from fixed prefix and suffix plus the mapped value, and calls `dlopen`; `createPen` resolves `createPenInst` as documented in v267.

The static evidence establishes the packaged registry mapping and initialization code; it does not establish an actual application runtime registry, Android library load, or selection by an original saved page. The loop's last pair is zero in the file and has no relative relocation; its runtime treatment has not been established. No production output profile is warranted from these static observations.

The verifier checks that both string addresses fall within the first file-backed PT_LOAD, whose virtual address and file offset are both zero; it then uses the virtual address as the file offset. Run without Python optimization.

Reproduce the entry using the packaged ELF and its relative relocations:

```sh
readelf -rW scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so | rg '0000000000078ae8|0000000000078af0'
python3 tools/pencil2-worker-v268/verify.py
export LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu
scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump -d -C --start-address=0x4d8a0 --stop-address=0x4dc2c scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so
```

REA tools were unavailable in this session; the observations use local ELF and disassembly tools.

`python3 tools/pencil2-worker-v268/runtime.py` instruments the pinned v241 synthetic worker and asserts its loader and factory trace. Its native Drawing route requests `libSPenPencil2.so` with flag 2, then `createPenInst` on the host adapter's handle, and the native factory returns a non-null pen. The adapter supplies the Pencil2 factory address for that handle; it does not perform an Android `dlopen`. The loader callback rejects other library names, so this observed run did not enter a DefaultPen fallback. The input is the harness's synthetic ObjectStroke, not an original saved note. This confirms selection within that controlled loader and manager run, not actual application registry contents, native page rendering, or device output. The script preserves v241's behavioral assertions, while skipping its receipt equality check because the added trace changes the generated script hash. As documented by v241, host runtime and transitive harness dependencies are not fully pinned. The factory return address is environment-dependent and is meaningful only within this controlled run; its precise value is not evidence of a device address.
