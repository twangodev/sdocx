# Changelog

## [0.7.1](https://github.com/twangodev/sdocx/compare/sdocx-cli-v0.7.0...sdocx-cli-v0.7.1) (2026-09-25)


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.7.0 to 0.8.0

## [0.7.0](https://github.com/twangodev/sdocx/compare/sdocx-cli-v0.6.0...sdocx-cli-v0.7.0) (2026-09-25)


### Features

* **ink:** preserve and resolve saved pen rendering inputs ([119705d](https://github.com/twangodev/sdocx/commit/119705d834c6aa257d6981e84c2fac89d3326d9e))
* **render:** draw APK-backed dot backgrounds through shared SVG geometry ([125e7e6](https://github.com/twangodev/sdocx/commit/125e7e63b2221f57be8fc02ee6226a485ffc61b9))
* **web:** add vector PDF export ([#20](https://github.com/twangodev/sdocx/issues/20)) ([b7efb68](https://github.com/twangodev/sdocx/commit/b7efb6823f7f7f493bf412198e257c1101a8eac2))


### Bug Fixes

* include README in published crates ([eda7bdf](https://github.com/twangodev/sdocx/commit/eda7bdfeecf1e2b74ebf53157b831463a8b87557))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.6.0 to 0.7.0

## [0.6.0](https://github.com/twangodev/sdocx/compare/sdocx-cli-v0.5.0...sdocx-cli-v0.6.0) (2026-09-14)


### Features

* **cli:** add PDF export with Rust 1.92 renderer dependencies ([0e30581](https://github.com/twangodev/sdocx/commit/0e30581f7df56a187486a19cf6c09983785ea7fe))
* **cli:** mirror Samsung rich-text layout ([a56ac38](https://github.com/twangodev/sdocx/commit/a56ac3884c1a2ddfe50ac62082366d7095b22fa0))
* **cli:** report document integrity checks ([3a53921](https://github.com/twangodev/sdocx/commit/3a539211fd1bdb3a44654c5f4aa6abddae6babe2))
* **cli:** support explicit fonts for consistent PNG rendering ([5a2fbef](https://github.com/twangodev/sdocx/commit/5a2fbefd1ca094d960586ba8c8447380d40523a4))
* **cli:** surface parser diagnostics during conversion ([7c465db](https://github.com/twangodev/sdocx/commit/7c465db8c72027b60053e1b488f54e101df86f61))
* **layout:** separate visible pages from storage ([a551609](https://github.com/twangodev/sdocx/commit/a551609a27a2577e90a9d153427a8b35debcbe39))
* **parser:** decode embedded text objects ([ab9b592](https://github.com/twangodev/sdocx/commit/ab9b5922118412d2b6f3b5dbd2081cf2df51f21b))
* **parser:** decode rich-text hyperlinks ([a42f892](https://github.com/twangodev/sdocx/commit/a42f892ff64f899e62ee05642b7c7cf2db8463de))
* **parser:** use stored text page sections ([b9a1504](https://github.com/twangodev/sdocx/commit/b9a1504c8508b3852ca5c56642b3bf8bf0e357d0))


### Bug Fixes

* **cli:** keep tests after runtime items ([05a7f4f](https://github.com/twangodev/sdocx/commit/05a7f4fbb587b79a0fb25c51f7f9a49e97654215))
* **cli:** match Samsung PDF styling ([d8ed489](https://github.com/twangodev/sdocx/commit/d8ed489b65c74daba03124dd846d1526b1bdef76))
* **cli:** render dark-mode text in PNG output ([1130344](https://github.com/twangodev/sdocx/commit/113034470dc8f33815eefec5349d776a38c1136e))
* **parser:** decode packed stroke channels correctly ([42e4a29](https://github.com/twangodev/sdocx/commit/42e4a29a3a0a59a59c0d1831d5569839535c349b))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.5.0 to 0.6.0

## [0.5.0](https://github.com/twangodev/sdocx/compare/sdocx-cli-v0.4.0...sdocx-cli-v0.5.0) (2026-05-26)


### Features

* **cli:** add Format enum and converter-style format resolution ([af80b07](https://github.com/twangodev/sdocx/commit/af80b07fd8b095dbc0c9470ae37ad90578cee3af))
* **cli:** rasterize SVG to PNG via resvg ([a1987fa](https://github.com/twangodev/sdocx/commit/a1987fa08dba59417ebc54d05860bd21d3fc3bc5))
* **cli:** select output format from extension or --format flag ([841de57](https://github.com/twangodev/sdocx/commit/841de57fed44ff76dfd5317a2d38110f63875d49))
* support Samsung Notes v4.4.x page format ([#9](https://github.com/twangodev/sdocx/issues/9)) ([7f89bda](https://github.com/twangodev/sdocx/commit/7f89bda394e70e3110642988cc2620ca6dda69ad))


### Bug Fixes

* **cli:** infer output format case-insensitively from extension ([e36b644](https://github.com/twangodev/sdocx/commit/e36b644ca1a21ba9e369c10aab21ecf9e2162b68))
* **cli:** make SVG rendering mode-aware (background + default ink) ([4e8243d](https://github.com/twangodev/sdocx/commit/4e8243d7b6df92e897e8f832e8d7909af3f68e0e))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.4.0 to 0.5.0

## [0.4.0](https://github.com/twangodev/sdocx/compare/sdocx-cli-v0.3.1...sdocx-cli-v0.4.0) (2026-03-09)


### Features

* enhance page parsing and SVG rendering for .sdocx files ([a5159b0](https://github.com/twangodev/sdocx/commit/a5159b0ba42638c6e5f43e22f410fafa79d4f1c5))


### Bug Fixes

* streamline output handling and improve formatting in main.rs and page.rs ([61c0f82](https://github.com/twangodev/sdocx/commit/61c0f82bfe8ce6c136c8fb37c539924b970a5923))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.3.1 to 0.4.0

## [0.3.1](https://github.com/twangodev/sdocx/compare/sdocx-cli-v0.3.0...sdocx-cli-v0.3.1) (2026-03-08)


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.3.0 to 0.3.1

## [0.3.0](https://github.com/twangodev/sdocx/compare/sdocx-cli-v0.2.0...sdocx-cli-v0.3.0) (2026-03-08)


### Features

* Add Docker support and CI/CD configuration for sdocx and sdocx-cli ([45dbc0a](https://github.com/twangodev/sdocx/commit/45dbc0a0b9ddb7bb6174a94475420598703de304))
* Add project metadata including description, license, and repository URL in Cargo.toml ([e98d5dc](https://github.com/twangodev/sdocx/commit/e98d5dcb9ba98a8f73f7f1eaca4bbd797ea0ed38))
* Implement parsing for .sdocx files; add container and decoding logic ([e4ace13](https://github.com/twangodev/sdocx/commit/e4ace1365bc085564d0be0054498b04719927d1f))
* Initialize Rust project with sdocx and sdocx-cli; add basic CLI functionality ([e6ce464](https://github.com/twangodev/sdocx/commit/e6ce4645e3010fe48e2ead9db0a3d2425686d90e))
* Update sdocx dependency to version 0.1.0 in Cargo.toml ([bf64fe9](https://github.com/twangodev/sdocx/commit/bf64fe99f2fb0a0493b07488c8f557137ee0e721))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.2.0 to 0.3.0

## [0.2.0](https://github.com/twangodev/sdocx/compare/sdocx-cli-v0.1.0...sdocx-cli-v0.2.0) (2026-03-08)


### Features

* Add Docker support and CI/CD configuration for sdocx and sdocx-cli ([45dbc0a](https://github.com/twangodev/sdocx/commit/45dbc0a0b9ddb7bb6174a94475420598703de304))
* Add project metadata including description, license, and repository URL in Cargo.toml ([e98d5dc](https://github.com/twangodev/sdocx/commit/e98d5dcb9ba98a8f73f7f1eaca4bbd797ea0ed38))
* Implement parsing for .sdocx files; add container and decoding logic ([e4ace13](https://github.com/twangodev/sdocx/commit/e4ace1365bc085564d0be0054498b04719927d1f))
* Initialize Rust project with sdocx and sdocx-cli; add basic CLI functionality ([e6ce464](https://github.com/twangodev/sdocx/commit/e6ce4645e3010fe48e2ead9db0a3d2425686d90e))
* Update sdocx dependency to version 0.1.0 in Cargo.toml ([bf64fe9](https://github.com/twangodev/sdocx/commit/bf64fe99f2fb0a0493b07488c8f557137ee0e721))


### Dependencies

* The following workspace dependencies were updated
  * dependencies
    * sdocx bumped from 0.1.0 to 0.2.0
