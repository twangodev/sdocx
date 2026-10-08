# Changelog

## [1.0.0](https://github.com/twangodev/sdocx/compare/sdocx-v0.7.0...sdocx-v1.0.0) (2026-10-08)


### ⚠ BREAKING CHANGES

* **api:** construct RenderedPage, PdfOutput and PdfPageDiagnostics with their new constructors instead of external struct literals. Matches on text and object diagnostic kinds must include a fallback arm.
* **render:** PdfPageDiagnostics includes source_page_index, geometry_diagnostics and paint_diagnostics; use its constructor when creating reports.
* **render:** RenderedPage includes geometry_diagnostics and paint_diagnostics. Older serialized payloads continue to deserialize with empty diagnostic vectors; Rust callers use the new constructor.
* **parser:** exhaustive ParseOptions and StoredNote literals must include the new retain_note_source and source_bytes fields.
* **parser:** ParseOptions and StoredArchivePage add fields; update exhaustive struct literals and patterns.
* **table:** Rust callers must use min_column_width and min_row_height. Serialization emits these names; deserialization accepts the previous padding-named keys. Raw optional values and rendering behavior are unchanged.

### Features

* add typed Rust paint metrics for static fonts ([38c8d6b](https://github.com/twangodev/sdocx/commit/38c8d6be1897304114abca5e391b402bf4bd9280))
* **api:** make render reports extensible ([85df6a9](https://github.com/twangodev/sdocx/commit/85df6a9229dc8c6ddc2f80c04f1462f33ba1c734))
* build native measured text pieces through one typed pipeline ([a0fb8a1](https://github.com/twangodev/sdocx/commit/a0fb8a1c01e8c395906c4d58c25d389ac5ad70d0))
* convert native paint layouts into typed logical entries ([25f6496](https://github.com/twangodev/sdocx/commit/25f6496055a18c07db94c8c81b1133e2e1c0da38))
* decode native composition and suggestion span payloads ([99c9736](https://github.com/twangodev/sdocx/commit/99c9736c458ba5b93fd2236f47786b8487a1bcfa))
* derive typed native paint profiles from source spans ([25f8829](https://github.com/twangodev/sdocx/commit/25f8829896be80ba112f7163348bd9445d9d3513))
* expose typed native paint shaping and layout ([a51aa6c](https://github.com/twangodev/sdocx/commit/a51aa6c336ed235cddac849fe7cbbe261958b100))
* index native measured text into typed page sections ([13bec7a](https://github.com/twangodev/sdocx/commit/13bec7add6911d3dbcd6e763ec7a814fcb649499))
* integrate certified native body pages and vector cell clips ([44dffdd](https://github.com/twangodev/sdocx/commit/44dffdde5c4fc06d74699c085c262e742b3ca76d))
* itemize native paint ranges with plain script properties ([c996031](https://github.com/twangodev/sdocx/commit/c9960318d903c6ce91e1797ddb45be69b87f3424))
* **layout:** retain validated full-source body reflow context ([28a5238](https://github.com/twangodev/sdocx/commit/28a5238a274ec3dca9a4ab7771a9556f650d6027))
* measure and export native table cell images ([e08912f](https://github.com/twangodev/sdocx/commit/e08912fe015e6df1c0ee3d3c4902a3897adb85a2))
* model native cached text run emission in Rust ([f0e8251](https://github.com/twangodev/sdocx/commit/f0e82513bb649d353de856849abf284b2d1fa325))
* package a bounded native font scaling backend ([1c4d897](https://github.com/twangodev/sdocx/commit/1c4d89787cbf3c01b1d75c51a43c2fb5b395ef7a))
* **parser:** optionally retain original page sources ([1dc2abd](https://github.com/twangodev/sdocx/commit/1dc2abd18666dff0384370d27a41ca308c469e13))
* **parser:** preserve legacy line pen source ([97b27f2](https://github.com/twangodev/sdocx/commit/97b27f2dc2d1c288d0e0607d35edc489d6a72ca9))
* **parser:** retain original note source on request ([8182f2a](https://github.com/twangodev/sdocx/commit/8182f2a3fa1cd3eebc4da304db086211d942624b))
* **pdf:** paint retained Rust glyph plans in document exports ([71bc17b](https://github.com/twangodev/sdocx/commit/71bc17b110ed9f21247bed3e2585e29fd149dbb1))
* preserve complete saved page background colors ([a264fa2](https://github.com/twangodev/sdocx/commit/a264fa2b196809eef195f5b6ab19df567ab4c3f9))
* preserve ordinary image fill source fields ([8cccbca](https://github.com/twangodev/sdocx/commit/8cccbcaf4970240c571db97c93bc8b4d5329cee7))
* preserve saved PDF paper source records ([11a285c](https://github.com/twangodev/sdocx/commit/11a285ca2e704a29d1034a95feb843c5bd42abec))
* preserve typed native span identity in retained text runs ([bb0978f](https://github.com/twangodev/sdocx/commit/bb0978f845d3df84a1d35cbd87c6c226a57b2cc9))
* preserve typed shape color and pattern sources ([fc4c99b](https://github.com/twangodev/sdocx/commit/fc4c99b9ce9646a1e3ecf396ca946fcf032dc064))
* render object background bands with native export policies ([269d841](https://github.com/twangodev/sdocx/commit/269d8414e287c7ea985cc9e54eb51d31e7b1fd0c))
* **render:** add independent paint diagnostic channel ([6e1a572](https://github.com/twangodev/sdocx/commit/6e1a572b39482014b1583618e977f2f2d80a8260))
* **render:** apply native shape gradient plans ([e641218](https://github.com/twangodev/sdocx/commit/e64121877c4cdd4fec45cab1dd8485dd84ff7696))
* **render:** derive typed native shape gradient plans ([8498fdd](https://github.com/twangodev/sdocx/commit/8498fdd6918e774c950763ce257aa35e1039e686))
* **render:** prepare dense merged tables with native frame owners ([178ae51](https://github.com/twangodev/sdocx/commit/178ae51658fb6f0d8efac23942fb8bd6d75a6fb6))
* **render:** preserve geometry diagnostics across exports ([f8a5638](https://github.com/twangodev/sdocx/commit/f8a5638183fd5b2754316f4c4d93b5e9aa63a288))
* **render:** report omitted native shape and line geometry ([7e684d3](https://github.com/twangodev/sdocx/commit/7e684d3557a2bd726939a9649293b70518dedc35))
* report unsupported native text styles across vector exports ([75f8efa](https://github.com/twangodev/sdocx/commit/75f8efadefc135089d2bf371fd50c306f8d57b53))
* reproduce native skew metrics from original font points ([208990a](https://github.com/twangodev/sdocx/commit/208990a68908dabca39c8434de51c75ff69d4c4d))
* resolve native composition styles through typed span patches ([2cb3fdd](https://github.com/twangodev/sdocx/commit/2cb3fddf7f625b8d15310ccbdf3ed3794003640a))
* resolve native font names and cache-word contexts ([4a5e67a](https://github.com/twangodev/sdocx/commit/4a5e67aa3e973a2e944e922ecb56e156e27a95a9))
* retain archive media source resources ([a007279](https://github.com/twangodev/sdocx/commit/a00727974d96bfe99137ba588d3e9f88ed29f672))
* retain bounded shape pen data slot ([f19d9ed](https://github.com/twangodev/sdocx/commit/f19d9ed6742bb90fcc12995a9353e91e4a26c9fa))
* retain common source metadata on decoded strokes ([5aa8aa1](https://github.com/twangodev/sdocx/commit/5aa8aa1bdd00e64ff2fc477c5d888bfdf8441d99))
* retain native bitmap font metadata on resolved faces ([a1bc311](https://github.com/twangodev/sdocx/commit/a1bc31155f7583ec9130f477e05fc24ae413023d))
* **sdocx:** preserve shape-base source data ([4d57446](https://github.com/twangodev/sdocx/commit/4d57446ffd76c53e97aaa367083d63c0051a5da3))
* **shape:** retain native orientation flags ([d7be495](https://github.com/twangodev/sdocx/commit/d7be49598252e37972cf9d89ef0ad1499c99de8e))
* stitch compatible native paint chunks in source order ([e9279ba](https://github.com/twangodev/sdocx/commit/e9279baf808cfe9e4bef96dd1e154d8790247c00))
* **table:** retain native row state across page layout retries ([f7cb92b](https://github.com/twangodev/sdocx/commit/f7cb92b21d8f2f4129fcc9fd44e509edac9705c4))
* **text:** apply native shape template frames ([d69012b](https://github.com/twangodev/sdocx/commit/d69012b41ce1e95d8737caca8b7900747be4b7d7))
* **text:** expose typed metadata for embedded object records ([615c56e](https://github.com/twangodev/sdocx/commit/615c56e593c2bf472effaa5c002cdc035fe2869b))
* **text:** implement native paragraph positioning ([cc7c884](https://github.com/twangodev/sdocx/commit/cc7c884c68064fb9a3a404f9bc991b7e04f6c884))
* **text:** paint background spans from retained native layout ([b014505](https://github.com/twangodev/sdocx/commit/b014505910111d271b474f5c18ae85aaa78b4bc2))
* **text:** position proven bidi lines from native paragraph maps ([0423eeb](https://github.com/twangodev/sdocx/commit/0423eebd156af5fa8d7d297db70e87ecb938e46b))
* **text:** preserve typed native span intervals ([7526e31](https://github.com/twangodev/sdocx/commit/7526e3184c6b8abef0518d263db2e54ceb6ca113))
* **text:** render unsectioned body text with measured reflow ([676cf6d](https://github.com/twangodev/sdocx/commit/676cf6d55f711b90d0582b621c6fc02dfb4f3ac0))
* **text:** restore native empty paragraph and cursor metrics ([cbe3593](https://github.com/twangodev/sdocx/commit/cbe3593822c80f9b64af8b66abdf11b2f8caa233))
* **text:** restore native path-dependent shape frames ([4d71a9d](https://github.com/twangodev/sdocx/commit/4d71a9df90a93468ff1001c9fe7fe47f012ae3db))
* **text:** retain covered non-Latin vector positions ([1a00342](https://github.com/twangodev/sdocx/commit/1a00342fa365c442737b48a0668d61fc42baf9db))
* **text:** retain native cold table cell layouts ([bb1179c](https://github.com/twangodev/sdocx/commit/bb1179cdc6ed4d84f0f291cc3ffe07e3a11de42e))
* use native text entries for paragraph layout and exports ([e166e53](https://github.com/twangodev/sdocx/commit/e166e53bb7c95ae5401a2b067658aeaf67689311))


### Bug Fixes

* apply hyperlink styling only to native action types ([6e6ea20](https://github.com/twangodev/sdocx/commit/6e6ea20d0866123fa2452a33013276f559b2503e))
* decode native CESU-8 font names ([24a1658](https://github.com/twangodev/sdocx/commit/24a16580bc9a47cd6683d3f6fcedd45bcef5cd5b))
* derive native document bounds and cell feedback state ([6f7ab72](https://github.com/twangodev/sdocx/commit/6f7ab729576a4992d64864b9bc00c3830f74d9d3))
* diagnose unrepresentable table border transforms ([74fb08c](https://github.com/twangodev/sdocx/commit/74fb08c7d5f82a03f5b47ed0333be0cda626fb70))
* include native font registry identity in body cache ([e93532b](https://github.com/twangodev/sdocx/commit/e93532b0646c60fcc5169d98e497d12016053942))
* **layout:** preserve ambiguous blank pages for body reflow ([783d52c](https://github.com/twangodev/sdocx/commit/783d52c6c945b989d44e9ba816d3c1d845797de6))
* **layout:** preserve caret spans and malformed source identity ([04b0a91](https://github.com/twangodev/sdocx/commit/04b0a912b39524f6489fce7861c4baeeca9f64c0))
* match captured table artwork transforms ([1c94aa8](https://github.com/twangodev/sdocx/commit/1c94aa8ce0853a064e9064d4b77f03b49278c332))
* match native code header and page reservation geometry ([f00ac89](https://github.com/twangodev/sdocx/commit/f00ac89453142eb4dc76cd3f678f4459a052adc0))
* match native float arithmetic in text line placement ([2230574](https://github.com/twangodev/sdocx/commit/22305740eaac796ebca22c9f188ace96e3d3d91b))
* match native table drawn-bound arithmetic ([f4e280e](https://github.com/twangodev/sdocx/commit/f4e280e0e1e39bd0071aecd539bccbcd1edcad8b))
* match the native mono target for hinted paint metrics ([55df4cc](https://github.com/twangodev/sdocx/commit/55df4cca5a5405d0e786110d5a64ff7bfdcf9b7c))
* **pdf:** order embedded text by logical source in tags ([276b3b2](https://github.com/twangodev/sdocx/commit/276b3b2cbe60e38d2f330cfc03bc73b4a129f539))
* **pdf:** reject omitted native document images ([e9028e2](https://github.com/twangodev/sdocx/commit/e9028e2b3618d7daac23b2a5c6361b351665e179))
* preserve certified cached text during table translation ([3a6212f](https://github.com/twangodev/sdocx/commit/3a6212fdd2c294413851b0faab2a5d90f34b99c1))
* preserve native cached text emission boundaries ([8ddb6eb](https://github.com/twangodev/sdocx/commit/8ddb6eb1d26d7a2c69eb4983e9b2857d0368af1d))
* preserve native cell model bounds during feedback ([95f023a](https://github.com/twangodev/sdocx/commit/95f023a902b4009f2d3926e13fefb88a80355bd7))
* preserve native heading spans and text producer rules ([dc410ec](https://github.com/twangodev/sdocx/commit/dc410ecafe0ab067701ee7854c7a280b2b6d4db9))
* preserve native object wrapping and alignment metrics ([f72874b](https://github.com/twangodev/sdocx/commit/f72874b3daee2025e4b855a894a3de6b0e1adeb1))
* preserve native table measurement and text placement ([67f2a14](https://github.com/twangodev/sdocx/commit/67f2a14ca5da88d7af72ce7044c33a79d952f4a4))
* preserve saved text ranges before exhausted pages ([561db72](https://github.com/twangodev/sdocx/commit/561db72cadfe3c66b8bfcba520adf8243bfcf1fe))
* preserve selected physical fonts in SVG and PDF ([211020e](https://github.com/twangodev/sdocx/commit/211020ec83d91fb31b2a5d4ad3f9c841f1abe173))
* **render:** accept saved table row maximum heights ([7674ed6](https://github.com/twangodev/sdocx/commit/7674ed66b9b40d3f1f985ed5622318132b3b6971))
* **render:** compress table rows using frame-owner caches ([5cb08e1](https://github.com/twangodev/sdocx/commit/5cb08e12d7f7d1b0abcbdc85719e14440706febf))
* **render:** derive table bounds and page minima from frame owners ([d562314](https://github.com/twangodev/sdocx/commit/d562314c6df3281ca81ff0761593a16c6e23c6d4))
* **render:** honor caret fonts for empty list markers ([0429b9a](https://github.com/twangodev/sdocx/commit/0429b9a9648b1bb69070746e74c4f6fc07e070a9))
* **render:** match native table export artwork crops ([74893c6](https://github.com/twangodev/sdocx/commit/74893c69d8e916d1771854bfe2b77d4e1e46add4))
* **render:** paint native table border geometry and styles ([832ab2c](https://github.com/twangodev/sdocx/commit/832ab2ca2183a17d1a46efed54f5c694693ff93e))
* **render:** prepare tables with saved height limits ([5299b66](https://github.com/twangodev/sdocx/commit/5299b66da8ec2a251ec204584487bb895afae399))
* **render:** preserve fractional shape geometry ([3f25dbf](https://github.com/twangodev/sdocx/commit/3f25dbfeef6ac08d9e65a1ddeea3332c80dce34a))
* **render:** preserve image placement precision ([700d14d](https://github.com/twangodev/sdocx/commit/700d14dda536c61afad68fbf79440473b39d4c94))
* **render:** preserve native merged table visibility ([2e4d1c5](https://github.com/twangodev/sdocx/commit/2e4d1c58f7dd1718cdd191174757e6c50a00a7ed))
* **render:** preserve native table fill inheritance and transparency ([12f2814](https://github.com/twangodev/sdocx/commit/12f2814ae8a9d5662e0aae2fc2e9c6bbcdad1424))
* **render:** preserve native table split cache ownership ([94882b9](https://github.com/twangodev/sdocx/commit/94882b975af839fab9d73ec9fb316e785de80a23))
* **render:** preserve saved vector coordinate precision ([37ae6b9](https://github.com/twangodev/sdocx/commit/37ae6b9e015b1a54d76cb960e3829e198634b68d))
* **render:** size warm table rows from cached frame owners ([92a7df1](https://github.com/twangodev/sdocx/commit/92a7df1f774f74f801310354c5cf8a7a9052606f))
* **render:** stabilize horizontal triangle directions ([6672969](https://github.com/twangodev/sdocx/commit/667296916e71cb577464fdd69cd6fac25f5bbb53))
* **render:** validate consumed gradient geometry ([8008f91](https://github.com/twangodev/sdocx/commit/8008f918fd3ee68c456bef5623f7ae452616d7e6))
* **render:** validate rotation through typed inputs ([f97b9d4](https://github.com/twangodev/sdocx/commit/f97b9d493722228250c5635d7920d92680fd9519))
* report unsupported dense table composition ([b22af76](https://github.com/twangodev/sdocx/commit/b22af760885db08ac096fbfe2291e582a9488f81))
* retain authoritative body layout with positive source bounds ([e848580](https://github.com/twangodev/sdocx/commit/e8485806d536b732622e9fe1c79f51e472cae95e))
* retain native cached geometry for combining marks ([84b0d2b](https://github.com/twangodev/sdocx/commit/84b0d2b3a8c01f02176803bee62f9033b67ef8b4))
* retain native defaults in parsed table cells ([fe357cc](https://github.com/twangodev/sdocx/commit/fe357cc386092068c4f52ad7ce38c0c24218ca6c))
* select composing backgrounds after theme mapping ([336020d](https://github.com/twangodev/sdocx/commit/336020dd97cf31fba0bf06dea03bd06920a889a5))
* separate text measurement identity from paint styles ([67946bc](https://github.com/twangodev/sdocx/commit/67946bcb31d2bd39976aa2d1a771d078ea139d3e))
* share native cached cell paint across vector exports ([80f832c](https://github.com/twangodev/sdocx/commit/80f832c871a2fddff1b5491f592177c9f6ad21f6))
* **svg:** embed the selected font collection face ([26fbb18](https://github.com/twangodev/sdocx/commit/26fbb18009930de77fe8f367f401bc8b0fa549c8))
* **svg:** keep synthesized styles within fallback spans ([ba48e22](https://github.com/twangodev/sdocx/commit/ba48e227963053eb4026bd1b561aaca3b0728b88))
* **table:** name native minimum size constraints correctly ([5f10c52](https://github.com/twangodev/sdocx/commit/5f10c521d08c82d99603834d5a39a6cac0398e00))
* **table:** regenerate native drawing frames at final origins ([d68254c](https://github.com/twangodev/sdocx/commit/d68254c4005b77528071e88745a1c75538e7204a))
* **text:** apply native body-flow object margins by layout context ([037527e](https://github.com/twangodev/sdocx/commit/037527ea7f538de0bc6f22853ff9ac89c66697fa))
* **text:** apply staged native object width feedback ([9ebef82](https://github.com/twangodev/sdocx/commit/9ebef8226c161c5d2c678a9e38610a67c5fd2a04))
* **text:** position embedded objects in native bidi order ([c5478aa](https://github.com/twangodev/sdocx/commit/c5478aa45f7210ec79f7b7f73d7a8f163252f773))
* **text:** preserve glyph positions and faces through local fallback ([f1bc6aa](https://github.com/twangodev/sdocx/commit/f1bc6aa897b68626395d2151b339b6364ad2852f))
* **text:** preserve native RTL defaults around isolates ([c2ec5f5](https://github.com/twangodev/sdocx/commit/c2ec5f538fd3d0c417e7d60acb308dc2a9678b04))
* **text:** recompute native margins at page boundaries ([0139478](https://github.com/twangodev/sdocx/commit/01394783dbd86831eab68e61d27124bb7f670a32))
* **text:** retain bidi context across wrapped lines ([d7ded25](https://github.com/twangodev/sdocx/commit/d7ded2501b0a72eaa7bf82cf0837d593a3b3b0cc))
* **text:** retain synthesized styles in vector PDF exports ([ef56a28](https://github.com/twangodev/sdocx/commit/ef56a285b40b2039261628bd9dd5d1091c22ee32))
* **text:** scope diagnostics and index style and bidi lookups ([6a8fe9a](https://github.com/twangodev/sdocx/commit/6a8fe9ac9110b08ba5d37a972c01d595ae6262cf))
* **text:** separate object metrics from final drawing geometry ([bf4a300](https://github.com/twangodev/sdocx/commit/bf4a300703a3039dbe481a6056bac79d4fa82056))


### Performance Improvements

* cache native wrapping topology across paragraph lines ([523b512](https://github.com/twangodev/sdocx/commit/523b51205428bb5a08c8160e2b495ff5ca17f364))
* **render:** reuse body plans across previews and exports ([bbe089d](https://github.com/twangodev/sdocx/commit/bbe089d491e5277d1d6c97112a69aaa8f64bfe26))

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
