The deterministic Roboto/DejaVu test collection is assembled in memory; generated
`Roboto-DejaVuSans.ttc` bytes are not tracked. It contains:

- Face 0: `crates/sdocx/assets/fonts/Roboto-Regular.ttf`, licensed under Apache 2.0; see `crates/sdocx/assets/fonts/Roboto-LICENSE.txt`.
- Face 1: `DejaVuSans.ttf` in this directory; see `LICENSE-dejavu.txt`.

`tests/helpers/font_collection.rs` assembles the collection from those unchanged
source tables. `font_collection_svg.rs` verifies the source hashes and collection
SHA-256 `05b90eb2586360bca28ff1852a12e0bda9933f344b0ae75b3465f22b73ed63ba`.
The browser fixture `web/tests/fixtures/font-collection.ts` uses the same assembly
and its Chromium test checks that identical hash before verifying selected-face
SVG embedding. Historical source inventories retain the original generated
filename and hash; their recorded bytes are reproducible from these sources.
