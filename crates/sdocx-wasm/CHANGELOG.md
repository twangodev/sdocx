# Changelog

## [1.1.0](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v1.0.0...sdocx-wasm-v1.1.0) (2026-10-09)


### Features

* report native decode and vector processing progress ([a79443d](https://github.com/twangodev/sdocx/commit/a79443d0cf8c3984dc1bba41742688dcab023be1))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 1.0.0 to 1.1.0

## [1.0.0](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v0.5.0...sdocx-wasm-v1.0.0) (2026-10-08)


### ⚠ BREAKING CHANGES

* **npm:** initialize `@twango/sdocx` with its default async `init()` export before calling `parse` or constructing `DocumentSession`. The package now uses the web-target module rather than automatic bundler initialization.
* **api:** construct RenderedPage, PdfOutput and PdfPageDiagnostics with their new constructors instead of external struct literals. Matches on text and object diagnostic kinds must include a fallback arm.
* **render:** PdfPageDiagnostics includes source_page_index, geometry_diagnostics and paint_diagnostics; use its constructor when creating reports.

### Features

* **api:** make render reports extensible ([85df6a9](https://github.com/twangodev/sdocx/commit/85df6a9229dc8c6ddc2f80c04f1462f33ba1c734))
* **pdf:** paint retained Rust glyph plans in document exports ([71bc17b](https://github.com/twangodev/sdocx/commit/71bc17b110ed9f21247bed3e2585e29fd149dbb1))
* **render:** preserve geometry diagnostics across exports ([f8a5638](https://github.com/twangodev/sdocx/commit/f8a5638183fd5b2754316f4c4d93b5e9aa63a288))
* summarize retained sources in browser inspection ([f48a48c](https://github.com/twangodev/sdocx/commit/f48a48cb11eef7d14e384bb40e78f0d80e3f1999))
* **text:** preserve typed native span intervals ([7526e31](https://github.com/twangodev/sdocx/commit/7526e3184c6b8abef0518d263db2e54ceb6ca113))
* transport paint diagnostics through export and preview reports ([338aba4](https://github.com/twangodev/sdocx/commit/338aba418205c1c68b9827019f377a802a6f4aba))
* **wasm:** expose detailed render reports ([f2ddc43](https://github.com/twangodev/sdocx/commit/f2ddc4328f7fc54f7a5a127ece07c6dc157b7d26))


### Bug Fixes

* preserve selected physical fonts in SVG and PDF ([211020e](https://github.com/twangodev/sdocx/commit/211020ec83d91fb31b2a5d4ad3f9c841f1abe173))
* preserve unsafe integers in browser inspection ([be0817c](https://github.com/twangodev/sdocx/commit/be0817cebf2d6ab4ebae8ebb6cd76dfa95bc4089))


### Performance Improvements

* **render:** reuse body plans across previews and exports ([bbe089d](https://github.com/twangodev/sdocx/commit/bbe089d491e5277d1d6c97112a69aaa8f64bfe26))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.7.0 to 1.0.0

## [0.5.0](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v0.4.0...sdocx-wasm-v0.5.0) (2026-09-25)


### Features

* **ink:** preserve and resolve saved pen rendering inputs ([119705d](https://github.com/twangodev/sdocx/commit/119705d834c6aa257d6981e84c2fac89d3326d9e))
* **ink:** reconstruct native FountainPen V16 stroke geometry ([c661033](https://github.com/twangodev/sdocx/commit/c661033bdd943a0ce3a0f814863e340fdffe528c))
* **render:** draw APK-backed dot backgrounds through shared SVG geometry ([125e7e6](https://github.com/twangodev/sdocx/commit/125e7e63b2221f57be8fc02ee6226a485ffc61b9))
* **wasm:** expose lazy document debugging and stroke replay data ([428e951](https://github.com/twangodev/sdocx/commit/428e951aac21602a3a88ba1b1710679a41a794b7))
* **web:** add vector PDF export ([#20](https://github.com/twangodev/sdocx/issues/20)) ([b7efb68](https://github.com/twangodev/sdocx/commit/b7efb6823f7f7f493bf412198e257c1101a8eac2))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.6.0 to 0.7.0

## [0.4.0](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v0.3.3...sdocx-wasm-v0.4.0) (2026-09-14)


### Features

* **cli:** add PDF export with Rust 1.92 renderer dependencies ([0e30581](https://github.com/twangodev/sdocx/commit/0e30581f7df56a187486a19cf6c09983785ea7fe))
* **parser:** verify document integrity relationships ([50e0342](https://github.com/twangodev/sdocx/commit/50e03425342d7a577bd13d280705fd18ce134b9e))


### Performance Improvements

* **render:** reuse document layout in wasm ([8a72b73](https://github.com/twangodev/sdocx/commit/8a72b73a42ab94f30511dda1df86beebf57cef83))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.5.0 to 0.6.0

## [0.3.3](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v0.3.2...sdocx-wasm-v0.3.3) (2026-05-26)


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.4.0 to 0.5.0

## [0.3.2](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v0.3.1...sdocx-wasm-v0.3.2) (2026-03-09)


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.3.1 to 0.4.0

## [0.3.1](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v0.3.0...sdocx-wasm-v0.3.1) (2026-03-08)


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.3.0 to 0.3.1

## [0.3.0](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v0.2.0...sdocx-wasm-v0.3.0) (2026-03-08)


### Features

* Add sdocx-wasm crate with WASM bindings and update dependencies ([2b55ec6](https://github.com/twangodev/sdocx/commit/2b55ec67a18a05855403b296694cd93b2d4eb620))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.2.0 to 0.3.0
