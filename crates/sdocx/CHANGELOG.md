# Changelog

## [0.6.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.5.0...sdocx-v0.6.0) (2026-09-06)


### Features

* **cli:** add PDF export with Rust 1.92 renderer dependencies ([0e30581](https://github.com/twangodev/sdocx/commit/0e30581f7df56a187486a19cf6c09983785ea7fe))
* **cli:** mirror Samsung rich-text layout ([a56ac38](https://github.com/twangodev/sdocx/commit/a56ac3884c1a2ddfe50ac62082366d7095b22fa0))
* **layout:** separate visible pages from storage ([a551609](https://github.com/twangodev/sdocx/commit/a551609a27a2577e90a9d153427a8b35debcbe39))
* **model:** expose SDK object and media identifiers ([1846d74](https://github.com/twangodev/sdocx/commit/1846d74a3a0964c226de3f2f3f624ab103b983e1))
* **parser:** add APK-aligned page storage model ([9b622e2](https://github.com/twangodev/sdocx/commit/9b622e21ade5377f2ad80c2bdea29448a3a75c5e))
* **parser:** add bounded native object frame reader ([2788daa](https://github.com/twangodev/sdocx/commit/2788daac0da871ceb8e5aa8d933e68d8bb9fc420))
* **parser:** add versioned bounded archive parsing ([70108de](https://github.com/twangodev/sdocx/commit/70108de551e49b3e247b60fa1a0a4bfb5ef01e1c))
* **parser:** decode and render native shapes and lines ([7523fa1](https://github.com/twangodev/sdocx/commit/7523fa1e9d70db2f1ae657bc7283d9b3d47e0ac2))
* **parser:** decode bounded media manifest records ([878d07c](https://github.com/twangodev/sdocx/commit/878d07c1f4d463dd078c39c9082d135a4c65ba99))
* **parser:** decode common object bundles and optional fields ([c7761a4](https://github.com/twangodev/sdocx/commit/c7761a426aba1fe85db3bd6acddbbdc088441c9c))
* **parser:** decode embedded text objects ([ab9b592](https://github.com/twangodev/sdocx/commit/ab9b5922118412d2b6f3b5dbd2081cf2df51f21b))
* **parser:** decode native formula metadata and strokes ([a44a98a](https://github.com/twangodev/sdocx/commit/a44a98ab66f7978191557b4454418172536b941f))
* **parser:** decode native table styles and borders ([e29aed2](https://github.com/twangodev/sdocx/commit/e29aed2cdf8a7aa48e6f8f2d94b5e965cf1f35ad))
* **parser:** decode native text area modes ([85278e9](https://github.com/twangodev/sdocx/commit/85278e93ee19eea089c062d3b4c962ae725b2beb))
* **parser:** decode optional document metadata ([621ce07](https://github.com/twangodev/sdocx/commit/621ce071141f7487531483e73052c8dd340a7ed3))
* **parser:** decode rich-text hyperlinks ([a42f892](https://github.com/twangodev/sdocx/commit/a42f892ff64f899e62ee05642b7c7cf2db8463de))
* **parser:** decode rich-text paragraphs ([142b53c](https://github.com/twangodev/sdocx/commit/142b53c015ff4d2dac79bd3683bc193c37b914bb))
* **parser:** decode standalone text boxes from native frames ([d52d2b8](https://github.com/twangodev/sdocx/commit/d52d2b8088083d1abc4de16b58dccf0c56873017))
* **parser:** decode structured note rich text ([3a8fe1b](https://github.com/twangodev/sdocx/commit/3a8fe1b443c3568b42afa08eba4a9f01e294c2c7))
* **parser:** expose common object properties and extensions ([5a2bd6c](https://github.com/twangodev/sdocx/commit/5a2bd6c7be17ffe8692126bf6f7ff49b810f5c08))
* **parser:** expose native layer identity and style metadata ([055f055](https://github.com/twangodev/sdocx/commit/055f0557b477518e5f4df702521955592fc68fe9))
* **parser:** expose native stroke properties and pen metadata ([bff0bb0](https://github.com/twangodev/sdocx/commit/bff0bb0a18a830ac83846a4dd71cb06e8ca53493))
* **parser:** expose ordered archive structure ([9d6cf1b](https://github.com/twangodev/sdocx/commit/9d6cf1b72884d2e340c33ec437fdee6efe89dcc2))
* **parser:** expose saved text-span object snapshots ([5718de0](https://github.com/twangodev/sdocx/commit/5718de04cde637a8307341227ec7e89c291eb300))
* **parser:** identify formula graph endpoints and stroke indices ([a428f0b](https://github.com/twangodev/sdocx/commit/a428f0b938cdafe4ab0ac69e4f335f7f88b2a18d))
* **parser:** inspect document protection appendices ([a67169f](https://github.com/twangodev/sdocx/commit/a67169ff25282c3417ae8520520d4d802cf91c56))
* **parser:** inspect native math object envelopes ([3c6e15d](https://github.com/twangodev/sdocx/commit/3c6e15d8ec37932c1f1ba5e2955e0ec48f04d430))
* **parser:** inspect native plot expressions and styles ([e301808](https://github.com/twangodev/sdocx/commit/e301808557e03ffdcb7552f5ff36dbf404614c14))
* **parser:** name native formula label relations ([a761f34](https://github.com/twangodev/sdocx/commit/a761f3430739c0bbb70ddb53fc67c0cba68f66a2))
* **parser:** name native object layout flags and resize modes ([ada52c2](https://github.com/twangodev/sdocx/commit/ada52c26f54eb4b33c7add49ee92c536884d83af))
* **parser:** name native object render layers ([7f92b22](https://github.com/twangodev/sdocx/commit/7f92b220529cd6e8fcd6933c18d1dedae8c4056f))
* **parser:** resolve native image placements through media bind IDs ([e685717](https://github.com/twangodev/sdocx/commit/e685717510ca8ebc1bc928e95ec1eae37325899a))
* **parser:** use stored text page sections ([b9a1504](https://github.com/twangodev/sdocx/commit/b9a1504c8508b3852ca5c56642b3bf8bf0e357d0))
* **parser:** verify document integrity relationships ([50e0342](https://github.com/twangodev/sdocx/commit/50e03425342d7a577bd13d280705fd18ce134b9e))
* **pdf:** export SVG pages as multipage vector PDFs ([7d82d38](https://github.com/twangodev/sdocx/commit/7d82d38ee5563baea5f33037c771c3b6a9515171))


### Bug Fixes

* **parser:** bound embedded rich text and report extensions ([d1b63be](https://github.com/twangodev/sdocx/commit/d1b63beb5bc732c7dd73f2b79f3bc65e5f8252bf))
* **parser:** bound note headers by native field offsets ([11216af](https://github.com/twangodev/sdocx/commit/11216aff4875428ee8babb62f507d5d469a58eae))
* **parser:** bound table row and cell fixed data ([47d93af](https://github.com/twangodev/sdocx/commit/47d93afce3b08dde0033457f42f74ca0ffae467f))
* **parser:** correct stroke pen identity references ([3a746e8](https://github.com/twangodev/sdocx/commit/3a746e8e6fd398d68cceb1fb0d74ebe175ca0303))
* **parser:** decode packed stroke channels correctly ([42e4a29](https://github.com/twangodev/sdocx/commit/42e4a29a3a0a59a59c0d1831d5569839535c349b))
* **parser:** decode strokes through stored object frames ([f67e205](https://github.com/twangodev/sdocx/commit/f67e2058aa8f63d1c1dbfd98f77d3978c517c160))
* **parser:** decode variable-length WDoc end tags ([35a616d](https://github.com/twangodev/sdocx/commit/35a616d07cb4c7cf0c4e01fb541f1ce614ab53d7))
* **parser:** honor appended document metadata ([77fc993](https://github.com/twangodev/sdocx/commit/77fc99318809175c639c4fb1e08b7430fa6e59d4))
* **parser:** honor the saved current physical layer ([577b065](https://github.com/twangodev/sdocx/commit/577b065917a177962fb0485ad9adf56ba214bb81))
* **parser:** omit hidden objects from visible pages ([b6842b6](https://github.com/twangodev/sdocx/commit/b6842b6dd7b5531024ab418c795a7e7e3fc87e7c))
* **parser:** preserve native pen references and bound shape paths ([6dfa981](https://github.com/twangodev/sdocx/commit/6dfa9817f883a072acc9a8af5fcad8199e6c6cd1))
* **parser:** report recognized objects without semantic decoding ([7b1f433](https://github.com/twangodev/sdocx/commit/7b1f4336648cf43c3989adb5495d02a1e91beca6))
* **parser:** stop inferring optional stylus channels ([8be4a5e](https://github.com/twangodev/sdocx/commit/8be4a5e4b1a7290ccea6ef8734b71c10b37e5d1a))
* **parser:** validate declared stroke point counts ([7da4d63](https://github.com/twangodev/sdocx/commit/7da4d63c41ac0c7993fba385bb3705fd9df36809))
* **render:** sanitize SVG hyperlink targets ([8c600b7](https://github.com/twangodev/sdocx/commit/8c600b76edee83be8cf262f37534bb33f186ef61))


### Performance Improvements

* **render:** reuse document layout in wasm ([8a72b73](https://github.com/twangodev/sdocx/commit/8a72b73a42ab94f30511dda1df86beebf57cef83))

## [0.5.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.4.0...sdocx-v0.5.0) (2026-05-26)


### Features

* support Samsung Notes v4.4.x page format ([#9](https://github.com/twangodev/sdocx/issues/9)) ([7f89bda](https://github.com/twangodev/sdocx/commit/7f89bda394e70e3110642988cc2620ca6dda69ad))

## [0.4.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.3.1...sdocx-v0.4.0) (2026-03-09)


### Features

* enhance page parsing and SVG rendering for .sdocx files ([a5159b0](https://github.com/twangodev/sdocx/commit/a5159b0ba42638c6e5f43e22f410fafa79d4f1c5))


### Bug Fixes

* streamline output handling and improve formatting in main.rs and page.rs ([61c0f82](https://github.com/twangodev/sdocx/commit/61c0f82bfe8ce6c136c8fb37c539924b970a5923))

## [0.3.1](https://github.com/twangodev/sdocx/compare/sdocx-v0.3.0...sdocx-v0.3.1) (2026-03-08)


### Bug Fixes

* add missing newlines at end of files in error.rs and types.rs ([0eaa711](https://github.com/twangodev/sdocx/commit/0eaa7116c42c55d2cf8d836ff3dc91c4780a497a))

## [0.3.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.2.0...sdocx-v0.3.0) (2026-03-08)


### Features

* Add Docker support and CI/CD configuration for sdocx and sdocx-cli ([45dbc0a](https://github.com/twangodev/sdocx/commit/45dbc0a0b9ddb7bb6174a94475420598703de304))
* Add project metadata including description, license, and repository URL in Cargo.toml ([e98d5dc](https://github.com/twangodev/sdocx/commit/e98d5dcb9ba98a8f73f7f1eaca4bbd797ea0ed38))
* Add sdocx-wasm crate with WASM bindings and update dependencies ([2b55ec6](https://github.com/twangodev/sdocx/commit/2b55ec67a18a05855403b296694cd93b2d4eb620))
* Implement parsing for .sdocx files; add container and decoding logic ([e4ace13](https://github.com/twangodev/sdocx/commit/e4ace1365bc085564d0be0054498b04719927d1f))
* Initialize Rust project with sdocx and sdocx-cli; add basic CLI functionality ([e6ce464](https://github.com/twangodev/sdocx/commit/e6ce4645e3010fe48e2ead9db0a3d2425686d90e))

## [0.2.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.1.0...sdocx-v0.2.0) (2026-03-08)


### Features

* Add Docker support and CI/CD configuration for sdocx and sdocx-cli ([45dbc0a](https://github.com/twangodev/sdocx/commit/45dbc0a0b9ddb7bb6174a94475420598703de304))
* Add project metadata including description, license, and repository URL in Cargo.toml ([e98d5dc](https://github.com/twangodev/sdocx/commit/e98d5dcb9ba98a8f73f7f1eaca4bbd797ea0ed38))
* Implement parsing for .sdocx files; add container and decoding logic ([e4ace13](https://github.com/twangodev/sdocx/commit/e4ace1365bc085564d0be0054498b04719927d1f))
* Initialize Rust project with sdocx and sdocx-cli; add basic CLI functionality ([e6ce464](https://github.com/twangodev/sdocx/commit/e6ce4645e3010fe48e2ead9db0a3d2425686d90e))
