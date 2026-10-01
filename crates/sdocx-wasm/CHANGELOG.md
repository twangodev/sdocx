# Changelog

## [1.0.0](https://github.com/twangodev/sdocx/compare/sdocx-wasm-v0.5.0...sdocx-wasm-v1.0.0) (2026-10-01)


### ⚠ BREAKING CHANGES

* **model:** Page owns objects instead of strokes and elements; serialized pages use the same object tree.

### Features

* **ink:** add shared Rust fountain rasterization ([3dd2fca](https://github.com/twangodev/sdocx/commit/3dd2fca15854858bbe4d857f4832cce8aeb6645b))
* **layout:** retain native body text capture windows ([0d8219c](https://github.com/twangodev/sdocx/commit/0d8219cc153259ec472ff5daa14b3020deb13f53))
* **pdf:** paint retained Rust glyph plans in document exports ([71bc17b](https://github.com/twangodev/sdocx/commit/71bc17b110ed9f21247bed3e2585e29fd149dbb1))
* **text:** preserve typed native span intervals ([7526e31](https://github.com/twangodev/sdocx/commit/7526e3184c6b8abef0518d263db2e54ceb6ca113))
* **text:** share pinned Rust fonts and shaped metrics across exports ([198bf8c](https://github.com/twangodev/sdocx/commit/198bf8c3eb2fa2a47d42086187dd825f0ac5c0f2))
* **wasm:** share font resources across preview replay and PDF ([f84e3c6](https://github.com/twangodev/sdocx/commit/f84e3c692d415fdca3155e891eb678a10ddb8e2a))


### Bug Fixes

* **debugger:** derive replay hit targets from Rust composition ([aacf065](https://github.com/twangodev/sdocx/commit/aacf0658832d6df2dde9264f839e3e6401fa8ff6))
* **render:** preserve native object order within composition passes ([d903c27](https://github.com/twangodev/sdocx/commit/d903c270e466cb5ae56f6286142c5fdbae313abb))
* **render:** resolve paper and foreground defaults together ([b65f5b7](https://github.com/twangodev/sdocx/commit/b65f5b7befc184170f37e3f43e08e4940937d7b7))


### Performance Improvements

* **render:** reuse body plans across previews and exports ([bbe089d](https://github.com/twangodev/sdocx/commit/bbe089d491e5277d1d6c97112a69aaa8f64bfe26))


### Code Refactoring

* **model:** preserve page objects in one ordered Rust tree ([5089616](https://github.com/twangodev/sdocx/commit/508961678538b8a6c270e802a48035e4c703403a))


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
