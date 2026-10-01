# Bundled SVG converter

Sources are copied from `krilla-svg` 0.8.1, upstream commit
`3ffdf0588cf98050aad6edba51ca70162e1fb5b5`, directory `crates/krilla-svg/src`.
The published crate checksum is
`1237d7c37b16ca9fbc2e72dde13a10d321f9a44aceee35c062bc0a43bfc7ce16`.
Upstream `lib.rs` is this directory's `mod.rs`.

Local changes adapt imports, visibility, and match patterns to this Rust 2024
module and add a text callback
at the existing ordered traversal point. Callback failures stop further traversal,
unwind converter graphics state, and propagate to the PDF exporter. Without a
callback, conversion retains upstream behavior. Filter rasterization does not
invoke the text callback; the exporter validates retained-text completeness.

`LICENSE_APACHE`, `LICENSE_MIT`, and `NOTICE.md` are copied from the same commit
of <https://github.com/LaurenzV/krilla>. Upstream attributes the SVG converter to
`svg2pdf` under Apache 2.0; its acknowledgement is preserved in `NOTICE.md`.
