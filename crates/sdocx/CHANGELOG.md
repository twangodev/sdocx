# Changelog

## [1.0.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.7.0...sdocx-v1.0.0) (2026-10-01)


### ⚠ BREAKING CHANGES

* **table:** Rust callers must use min_column_width and min_row_height. Serialization emits these names; deserialization accepts the previous padding-named keys. Raw optional values and rendering behavior are unchanged.
* **model:** Page owns objects instead of strokes and elements; serialized pages use the same object tree.

### Features

* **ink:** add shared Rust fountain rasterization ([3dd2fca](https://github.com/twangodev/sdocx/commit/3dd2fca15854858bbe4d857f4832cce8aeb6645b))
* **layout:** retain native body text capture windows ([0d8219c](https://github.com/twangodev/sdocx/commit/0d8219cc153259ec472ff5daa14b3020deb13f53))
* **layout:** retain validated full-source body reflow context ([28a5238](https://github.com/twangodev/sdocx/commit/28a5238a274ec3dca9a4ab7771a9556f650d6027))
* **pdf:** paint retained Rust glyph plans in document exports ([71bc17b](https://github.com/twangodev/sdocx/commit/71bc17b110ed9f21247bed3e2585e29fd149dbb1))
* **shape:** retain native orientation flags ([d7be495](https://github.com/twangodev/sdocx/commit/d7be49598252e37972cf9d89ef0ad1499c99de8e))
* **svg:** add validated lists of text positions ([975d882](https://github.com/twangodev/sdocx/commit/975d882a60bfa5047f2fb08fd6c8cf706f97f16d))
* **table:** retain native row state across page layout retries ([f7cb92b](https://github.com/twangodev/sdocx/commit/f7cb92b21d8f2f4129fcc9fd44e509edac9705c4))
* **text:** apply native gravity to measured placed text plans ([8feeedd](https://github.com/twangodev/sdocx/commit/8feeedd5a024a9da2a77b664bec121df0157bdf9))
* **text:** apply native shape template frames ([d69012b](https://github.com/twangodev/sdocx/commit/d69012b41ce1e95d8737caca8b7900747be4b7d7))
* **text:** compose measured text and embedded objects in one stream ([7f90aa4](https://github.com/twangodev/sdocx/commit/7f90aa4122f1074e11cead5f8e6a0b713038dbd5))
* **text:** embed shared fonts in typed vector output ([774b543](https://github.com/twangodev/sdocx/commit/774b543d1d1892de00a5c91005fa8f3713da4896))
* **text:** expose typed metadata for embedded object records ([615c56e](https://github.com/twangodev/sdocx/commit/615c56e593c2bf472effaa5c002cdc035fe2869b))
* **text:** implement native paragraph positioning ([cc7c884](https://github.com/twangodev/sdocx/commit/cc7c884c68064fb9a3a404f9bc991b7e04f6c884))
* **text:** model vertical exclusions in shared text layout ([479e3a6](https://github.com/twangodev/sdocx/commit/479e3a65833627b040f54e73c728710756fd4568))
* **text:** paint background spans from retained native layout ([b014505](https://github.com/twangodev/sdocx/commit/b014505910111d271b474f5c18ae85aaa78b4bc2))
* **text:** position proven bidi lines from native paragraph maps ([0423eeb](https://github.com/twangodev/sdocx/commit/0423eebd156af5fa8d7d297db70e87ecb938e46b))
* **text:** preserve typed native span intervals ([7526e31](https://github.com/twangodev/sdocx/commit/7526e3184c6b8abef0518d263db2e54ceb6ca113))
* **text:** render native body capture windows as visible vectors ([a794529](https://github.com/twangodev/sdocx/commit/a79452970c65c4cb351dbd2679ff64a596908ba3))
* **text:** render unsectioned body text with measured reflow ([676cf6d](https://github.com/twangodev/sdocx/commit/676cf6d55f711b90d0582b621c6fc02dfb4f3ac0))
* **text:** resolve font fallback within the shared Rust database ([7533245](https://github.com/twangodev/sdocx/commit/75332450da5b94973b687f7af5f5b3e580bd4b93))
* **text:** restore native empty paragraph and cursor metrics ([cbe3593](https://github.com/twangodev/sdocx/commit/cbe3593822c80f9b64af8b66abdf11b2f8caa233))
* **text:** restore native path-dependent shape frames ([4d71a9d](https://github.com/twangodev/sdocx/commit/4d71a9df90a93468ff1001c9fe7fe47f012ae3db))
* **text:** retain covered non-Latin vector positions ([1a00342](https://github.com/twangodev/sdocx/commit/1a00342fa365c442737b48a0668d61fc42baf9db))
* **text:** retain native cold table cell layouts ([bb1179c](https://github.com/twangodev/sdocx/commit/bb1179cdc6ed4d84f0f291cc3ffe07e3a11de42e))
* **text:** share measured glyph placement across vector text contexts ([6c83ffa](https://github.com/twangodev/sdocx/commit/6c83ffaaed857cad41c541f123d0c4816cb046e3))
* **text:** share measured layout with table cells and code blocks ([38e4a1a](https://github.com/twangodev/sdocx/commit/38e4a1a76d83f83768d0ce64d0ad1dd2f07323c8))
* **text:** share pinned Rust fonts and shaped metrics across exports ([198bf8c](https://github.com/twangodev/sdocx/commit/198bf8c3eb2fa2a47d42086187dd825f0ac5c0f2))
* **text:** wrap Unicode paragraphs with measured Rust font runs ([9e8d2a5](https://github.com/twangodev/sdocx/commit/9e8d2a5c2947c05f817aeb0bca29a275a5758bcf))


### Bug Fixes

* **build:** gate display-line indexing on rendering ([1887ba1](https://github.com/twangodev/sdocx/commit/1887ba13f0be259fe52e5407d9363d9c07e272ce))
* **debugger:** derive replay hit targets from Rust composition ([aacf065](https://github.com/twangodev/sdocx/commit/aacf0658832d6df2dde9264f839e3e6401fa8ff6))
* **ink:** match native fountain input modes and saved widths ([63b73a7](https://github.com/twangodev/sdocx/commit/63b73a75e88e6f74f6d6f56eae6b23b711a009f5))
* **ink:** preserve native fountain stamp directions ([434df20](https://github.com/twangodev/sdocx/commit/434df20ac2e3f6c5e58a4119419eb7f9656f5426))
* **ink:** reconstruct Marker4 V7 highlighter vectors ([1c93776](https://github.com/twangodev/sdocx/commit/1c93776bb778023452d26913036096ef6fe0252a))
* **layout:** preserve ambiguous blank pages for body reflow ([783d52c](https://github.com/twangodev/sdocx/commit/783d52c6c945b989d44e9ba816d3c1d845797de6))
* **layout:** preserve caret spans and malformed source identity ([04b0a91](https://github.com/twangodev/sdocx/commit/04b0a912b39524f6489fce7861c4baeeca9f64c0))
* **layout:** validate native capture source snapshots ([98674a6](https://github.com/twangodev/sdocx/commit/98674a6335d2dbff580062a621ac410282934163))
* **model:** decode table cell editability from the native flag ([881aba6](https://github.com/twangodev/sdocx/commit/881aba6ef5d296d05eb4730f4f9d06a3d0caade2))
* **parser:** preserve unresolved object render selection ([8fb1007](https://github.com/twangodev/sdocx/commit/8fb1007f328e88cc34d5269bde01a71a4df58ee9))
* **pdf:** order embedded text by logical source in tags ([276b3b2](https://github.com/twangodev/sdocx/commit/276b3b2cbe60e38d2f330cfc03bc73b4a129f539))
* **render:** honor caret fonts for empty list markers ([0429b9a](https://github.com/twangodev/sdocx/commit/0429b9a9648b1bb69070746e74c4f6fc07e070a9))
* **render:** keep vector highlighters visible on dark paper ([6cdfef2](https://github.com/twangodev/sdocx/commit/6cdfef22696e6e187a3b1cfa86957893a59bbc0e))
* **render:** preserve composed SVG styles and verify typed output ([00ffccb](https://github.com/twangodev/sdocx/commit/00ffccb63fc2a6156f1d9d0656c6dc3cad613ad6))
* **render:** preserve native object order within composition passes ([d903c27](https://github.com/twangodev/sdocx/commit/d903c270e466cb5ae56f6286142c5fdbae313abb))
* **render:** resolve paper and foreground defaults together ([b65f5b7](https://github.com/twangodev/sdocx/commit/b65f5b7befc184170f37e3f43e08e4940937d7b7))
* **render:** shade V14 fountain strokes with vector gradients ([fae17ca](https://github.com/twangodev/sdocx/commit/fae17ca5f55ea8fa239871525cd14ee31ff2f5bb))
* **render:** share native color adaptation across vector content ([07b45e0](https://github.com/twangodev/sdocx/commit/07b45e008f287c099be35f65df1667c33e71e4f3))
* **render:** stabilize horizontal triangle directions ([6672969](https://github.com/twangodev/sdocx/commit/667296916e71cb577464fdd69cd6fac25f5bbb53))
* **svg:** embed the selected font collection face ([26fbb18](https://github.com/twangodev/sdocx/commit/26fbb18009930de77fe8f367f401bc8b0fa549c8))
* **svg:** keep synthesized styles within fallback spans ([ba48e22](https://github.com/twangodev/sdocx/commit/ba48e227963053eb4026bd1b561aaca3b0728b88))
* **table:** name native minimum size constraints correctly ([5f10c52](https://github.com/twangodev/sdocx/commit/5f10c521d08c82d99603834d5a39a6cac0398e00))
* **table:** regenerate native drawing frames at final origins ([d68254c](https://github.com/twangodev/sdocx/commit/d68254c4005b77528071e88745a1c75538e7204a))
* **text:** apply native body-flow object margins by layout context ([037527e](https://github.com/twangodev/sdocx/commit/037527ea7f538de0bc6f22853ff9ac89c66697fa))
* **text:** apply native embedded baselines and page exclusions ([c86c69b](https://github.com/twangodev/sdocx/commit/c86c69bd0688fc724e17ff1435d9b18d3993d9ac))
* **text:** apply native line spacing to text baselines ([f6df979](https://github.com/twangodev/sdocx/commit/f6df979b38a494c8ffae7e9d57f7ce0b5527f488))
* **text:** apply staged native object width feedback ([9ebef82](https://github.com/twangodev/sdocx/commit/9ebef8226c161c5d2c678a9e38610a67c5fd2a04))
* **text:** derive font sizing and spacing from native document density ([7efbd14](https://github.com/twangodev/sdocx/commit/7efbd14b090e7da0dc4a7b36f56ba4711e58d00a))
* **text:** feed prepared code heights into native object placement ([e2af3e0](https://github.com/twangodev/sdocx/commit/e2af3e00a5de4a093a8b52c8b64f1c011cf8f4bc))
* **text:** honor local font families and native line spacing ([a2cc167](https://github.com/twangodev/sdocx/commit/a2cc167a239762996881ec347b5e72239e8fce3b))
* **text:** honor native defaults and paragraph ordinals ([1425f6c](https://github.com/twangodev/sdocx/commit/1425f6c4b22d68b49818c26043d7e06c62cd1b4a))
* **text:** keep partial span styles out of whole-box defaults ([a2632a2](https://github.com/twangodev/sdocx/commit/a2632a299080f13ef1a6ed9c1461659d39356837))
* **text:** position embedded objects in native bidi order ([c5478aa](https://github.com/twangodev/sdocx/commit/c5478aa45f7210ec79f7b7f73d7a8f163252f773))
* **text:** prepare measured list markers through shared layout ([ea5c5a6](https://github.com/twangodev/sdocx/commit/ea5c5a6e25ff70222f6bf1911d4c0076679d77f5))
* **text:** preserve glyph positions and faces through local fallback ([f1bc6aa](https://github.com/twangodev/sdocx/commit/f1bc6aa897b68626395d2151b339b6364ad2852f))
* **text:** preserve native RTL defaults around isolates ([c2ec5f5](https://github.com/twangodev/sdocx/commit/c2ec5f538fd3d0c417e7d60acb308dc2a9678b04))
* **text:** preserve source when object geometry exceeds native bounds ([01c3c6c](https://github.com/twangodev/sdocx/commit/01c3c6c711b9492c06e7f2f348313f73dda2ca0f))
* **text:** preserve vector text when native style scaling overflows ([2686eda](https://github.com/twangodev/sdocx/commit/2686edaba9528beec3e073b08d82503d6f84455a))
* **text:** preserve wrapped spaces and lock native line positions ([0fdeed8](https://github.com/twangodev/sdocx/commit/0fdeed8fc74c3cd93c80db6216085cc534594391))
* **text:** recompute native margins at page boundaries ([0139478](https://github.com/twangodev/sdocx/commit/01394783dbd86831eab68e61d27124bb7f670a32))
* **text:** reject invalid runs before pagination clips them ([c31190e](https://github.com/twangodev/sdocx/commit/c31190e7641b3f4619ab6d955c4d039893e3c852))
* **text:** retain bidi context across wrapped lines ([d7ded25](https://github.com/twangodev/sdocx/commit/d7ded2501b0a72eaa7bf82cf0837d593a3b3b0cc))
* **text:** retain synthesized styles in vector PDF exports ([ef56a28](https://github.com/twangodev/sdocx/commit/ef56a285b40b2039261628bd9dd5d1091c22ee32))
* **text:** scale point marker reservations using native constants ([14acf13](https://github.com/twangodev/sdocx/commit/14acf135b8de8893e10ba0b455fbc50df2bbd40a))
* **text:** scope diagnostics and index style and bidi lookups ([6a8fe9a](https://github.com/twangodev/sdocx/commit/6a8fe9ac9110b08ba5d37a972c01d595ae6262cf))
* **text:** separate object metrics from final drawing geometry ([bf4a300](https://github.com/twangodev/sdocx/commit/bf4a300703a3039dbe481a6056bac79d4fa82056))
* **text:** share native cursors and render point markers as vectors ([62d44c7](https://github.com/twangodev/sdocx/commit/62d44c7c3fb7646cd170de79a00fb4029670eecd))


### Performance Improvements

* **render:** reuse body plans across previews and exports ([bbe089d](https://github.com/twangodev/sdocx/commit/bbe089d491e5277d1d6c97112a69aaa8f64bfe26))
* **text:** reuse native paragraph advances for line wrapping ([05392a7](https://github.com/twangodev/sdocx/commit/05392a714481dc3ed795c624179363628b0aaa3f))


### Code Refactoring

* **model:** preserve page objects in one ordered Rust tree ([5089616](https://github.com/twangodev/sdocx/commit/508961678538b8a6c270e802a48035e4c703403a))

## [0.7.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.6.0...sdocx-v0.7.0) (2026-09-25)


### Features

* **ink:** preserve and resolve saved pen rendering inputs ([119705d](https://github.com/twangodev/sdocx/commit/119705d834c6aa257d6981e84c2fac89d3326d9e))
* **ink:** reconstruct native FountainPen V16 stroke geometry ([c661033](https://github.com/twangodev/sdocx/commit/c661033bdd943a0ce3a0f814863e340fdffe528c))
* **ink:** reconstruct saved FountainPen V14 geometry ([f4c5ee5](https://github.com/twangodev/sdocx/commit/f4c5ee5e5639eb4697b2b1a80a006fea43e38662))
* **ink:** render saved Marker2 stamps with shared curve geometry ([7799784](https://github.com/twangodev/sdocx/commit/7799784623ce20ff2f8c939c87a50e59a7963f83))
* **render:** draw APK-backed dot backgrounds through shared SVG geometry ([125e7e6](https://github.com/twangodev/sdocx/commit/125e7e63b2221f57be8fc02ee6226a485ffc61b9))
* **render:** support native narrow medium and wide ruled templates ([e703a08](https://github.com/twangodev/sdocx/commit/e703a080d84c3307bc8093088ad076d0d1fae056))
* **wasm:** expose lazy document debugging and stroke replay data ([428e951](https://github.com/twangodev/sdocx/commit/428e951aac21602a3a88ba1b1710679a41a794b7))
* **web:** add vector PDF export ([#20](https://github.com/twangodev/sdocx/issues/20)) ([b7efb68](https://github.com/twangodev/sdocx/commit/b7efb6823f7f7f493bf412198e257c1101a8eac2))


### Bug Fixes

* include README in published crates ([eda7bdf](https://github.com/twangodev/sdocx/commit/eda7bdfeecf1e2b74ebf53157b831463a8b87557))
* **ink:** preserve Marker4 width opacity and rectangular tips ([8cd54ee](https://github.com/twangodev/sdocx/commit/8cd54ee9ea3c6c6953cdc6f2489ab0a31e3b8511))
* **layout:** recognize list-mode compatibility pages without body text ([3ea505e](https://github.com/twangodev/sdocx/commit/3ea505ef0be1e772edd584d7a7e5a0c70a2411f3))
* **parser:** decode native shape text editability ([f6c0be7](https://github.com/twangodev/sdocx/commit/f6c0be7603e18208af10954da0909d94444e77b4))
* **render:** reuse saved native paths for shape geometry ([5db7a09](https://github.com/twangodev/sdocx/commit/5db7a09da45867089746f1365bd950b3cf810d05))
* **web:** composite replay highlights over page and ink ([28d7dbb](https://github.com/twangodev/sdocx/commit/28d7dbb4063a97ea68b3c558ce749dc9a90615c0))

## [0.6.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.5.0...sdocx-v0.6.0) (2026-09-14)


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
* render images embedded in note text flow ([acc7017](https://github.com/twangodev/sdocx/commit/acc7017b76af40bebddc6d0a86653b699667e7a2))


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
* recognize supported embedded image settings ([e21f3ad](https://github.com/twangodev/sdocx/commit/e21f3ad7c5f4928cec6e2d26247adc0dc838632c))
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
