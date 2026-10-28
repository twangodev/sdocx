# Synthetic saved Arc roundtrip

This probe constructs a native Model shape, sets type 20 and bounds `(10,20,210,120)`, writes a document-type-2 frame, and loads it into a second native shape with orientation 0 and caller width 1848. It checks that all 394 bytes are consumed and that the source and loaded bounds, controls, active outline and one type-1 sector fill match. The retained frame has SHA-256 `44a464f3f641e9ce7e80e455fab87dd8b87891f8bf458495b22981cf2d7dbb3e`. The native writer result register is not interpreted as a byte count.

```sh
tools/named_arc_saved/replay.sh /absolute/path/to/arm64-v8a /absolute/path/to/unicorn/lib
python3 tools/named_arc_saved/audit-bindings.py /absolute/path/to/arm64-v8a
```

Replay requires rustc edition 2024, `rg`, `sha256sum` and the libraries pinned by the probe. It checks the capture and two principal harness source hashes, Unicorn's runtime hash, the stored frame, and the compact receipt across fills `[0,85,165,255,0]`. Other included harness modules and the Rust toolchain are not source-pinned. The optional audit resolves the original ELF relocations and defined symbols; it does not check final guest GOT values or non-PLT indirect calls. The Model direct branch writes are asserted separately. PLT tracing is 16-aligned and does not record every instruction or dynamic target.

Two bounded bump allocators do not reuse storage. Their shared event ledger checks allocated addresses and frees across both arenas, but does not instrument native memory accesses for use-after-free. Deterministic sequential UUIDs, single-thread mutexes, TLS, logging, bounded memory/string services and pinned Linux libm are host substitutions. The emulator and host services do not prove Android behavior. This is one synthetic unrotated default shape, not a captured user document; it does not execute Drawing, canvas, pixels, whole-document loading, or establish device, SVG, or PDF parity.
