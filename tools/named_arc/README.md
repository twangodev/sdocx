# Named Arc native geometry replay

This synthetic control uses Samsung Notes 4.4.45.37 arm64 Model, Base, Skia and libc++ libraries. The native constructor, rectangle setter and two control setters execute, along with native Path, Base List and libc++ sort. It records paths after the rectangle and after both control setters; it does not record constructor-only geometry. It records active command fields only; inactive segment words depend on allocation contents. Four separate processes with allocation fills 0, 85, 165 and 255 must agree. Bounds are `(10,20,210,120)`, followed by control 0 `(20,30)` and control 1 `(40,100)`.

The capture pins Model SHA-256 `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`, Base `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb`, Skia `42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d`, and libc++ `4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4`. libc++ must be alongside Model. Host Linux libm functions, including pow, are checked against SHA-256 `6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3`. The local Unicorn shared library used here had SHA-256 `ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c`.

This requires rustc with edition 2024, Python 3, Unicorn with its development link and runtime soname available, sha256sum, and the four supplied libraries. For example, with an existing Unicorn directory and the libraries in `LIBDIR`:

```sh
rustc --edition=2024 conformance/native_table.rs -L "$UNICORN_DIR" -o /tmp/sdocx-named-arc-probe
LD_LIBRARY_PATH="$UNICORN_DIR" python3 tools/named_arc/replay.py /tmp/sdocx-named-arc-probe "$LIBDIR/libSPenModel.so" "$LIBDIR/libSPenBase.so" "$LIBDIR/libSPenSkia.so"
```

The replay compares the resulting observations with `conformance/named-arc-native.json`. Unbound imports trap. Host allocation, memory, libm, logging, filename, TLS and mutex substitutions are boundaries. This does not execute the saved-file loader, Drawing, canvas, or pixels and does not establish device parity. No independent toolchain replay has been performed. REA MCP tools and CLI were unavailable in this session; no machine-wide registration was created.
