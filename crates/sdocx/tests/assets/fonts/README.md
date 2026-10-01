`Roboto-DejaVuSans.ttc` is a deterministic test collection containing:

- Face 0: `crates/sdocx/assets/fonts/Roboto-Regular.ttf`, licensed under Apache 2.0; see `crates/sdocx/assets/fonts/Roboto-LICENSE.txt`.
- Face 1: `DejaVuSans.ttf` in this directory; see `LICENSE-dejavu.txt`.

`tests/helpers/font_collection.rs` assembles the collection from those unchanged source tables. `font_collection_svg.rs` verifies the source hashes and reproduces the collection byte for byte. Rust and Chromium tests share this fixture to verify that the selected collection face survives standalone SVG embedding.
