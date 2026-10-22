# Changelog

## [1.2.0](https://github.com/twangodev/sdocx/compare/sdocx-v1.1.0...sdocx-v1.2.0) (2026-10-22)


### Features

* **ink:** add bounded profile-aware InkPen2 V9 vectors ([5fa9f50](https://github.com/twangodev/sdocx/commit/5fa9f5006f879c2a85c77cae488080163e3de980))
* **ink:** add bounded stateful Calligraphy model adapter ([c45a4e5](https://github.com/twangodev/sdocx/commit/c45a4e55b57c47d4a888e77294258619ed49d045))
* **ink:** reconstruct bounded InkPen V1 page vectors ([6dd6aae](https://github.com/twangodev/sdocx/commit/6dd6aae8dc9db7d620ef92f2ad10272b31612dc6))
* **ink:** reconstruct bounded MagicPen V1 curved strokes ([63edd83](https://github.com/twangodev/sdocx/commit/63edd8371863e0c1aade4c61301e8f7011f26eeb))
* **ink:** reconstruct configured InkPen2 enhanced antialiasing ([3fb3b0d](https://github.com/twangodev/sdocx/commit/3fb3b0dbe30aba34314ce9e8336ee42b45430917))
* **ink:** reconstruct curved InkPen V1 tools 3 and 4 ([f0bb56c](https://github.com/twangodev/sdocx/commit/f0bb56cfa107de2060832faca4f27ea896d74008))
* **ink:** reconstruct InkPen V1 no-curve tools 3 and 4 ([f38045e](https://github.com/twangodev/sdocx/commit/f38045e1e21c7f23fb4c345dace506476c1e3f57))
* **ink:** reconstruct MagicPen V1 no-curve vector exports ([62661e7](https://github.com/twangodev/sdocx/commit/62661e74ae9cb67ad9ca0912060c36627c87bcd5))
* **ink:** reconstruct Oblique V1 tool1 strokes ([e0d7d0e](https://github.com/twangodev/sdocx/commit/e0d7d0e1e242dc905e683739beabb1ad0b41c88f))
* **ink:** reconstruct ordinary Oblique V1 tool3 strokes ([9503929](https://github.com/twangodev/sdocx/commit/9503929eaf5a53251e75558e16726ffc0c880198))
* **ink:** reconstruct test-only InkPen V1 sampling ([2aa7f48](https://github.com/twangodev/sdocx/commit/2aa7f48be06da78d461ae2a6caa84764eb13c8dd))
* **ink:** recover native InkPen2 gradient color generation ([f545c1c](https://github.com/twangodev/sdocx/commit/f545c1cfc472b3c06ea6984a15f0713436f0c69b))
* **ink:** render bounded native Pencil3 V3 texture profiles ([b0e4a08](https://github.com/twangodev/sdocx/commit/b0e4a0824233bc64f4e46cb951b306ac66b0b770))
* **ink:** render bounded Oblique V1 strokes as native meshes ([90e96c2](https://github.com/twangodev/sdocx/commit/90e96c2d68789ff780a7f968cac9c37b1f4e1b82))
* **ink:** replay Pencil3 particles by native source boundaries ([04fdc2d](https://github.com/twangodev/sdocx/commit/04fdc2db3a4a408b450c38f4652605d86f5360ec))
* **ink:** stage native InkPen2 gradient vector transport ([a68bf88](https://github.com/twangodev/sdocx/commit/a68bf8834b1db1f6971bccd65d5f7421c5d849b5))
* **pdf:** place overlays on cropped and rotated paper ([625c9ff](https://github.com/twangodev/sdocx/commit/625c9ff505948554ddd5f37dbd336c33387dcf44))
* **pdf:** preserve contiguous imported paper pages ([ad0d433](https://github.com/twangodev/sdocx/commit/ad0d433765043c4fa438a82ddf7ca980f2c9718c))
* **pdf:** preserve grayscale colorizer filters as vectors ([2d60fe7](https://github.com/twangodev/sdocx/commit/2d60fe7beb0374eb63107bebeebc4dbe1337ec10))
* **pdf:** preserve grayscale Multiply colorizers as vectors ([b5585d0](https://github.com/twangodev/sdocx/commit/b5585d00072dc887e288789d5de5c9e929effaee))
* **pdf:** preserve imported paper in continuous stroke notes ([ee6a9a6](https://github.com/twangodev/sdocx/commit/ee6a9a69d5ea0c9b8c5fc79e955f14a6bef077d2))
* **pdf:** preserve LF-separated body images over imported paper ([b38ef38](https://github.com/twangodev/sdocx/commit/b38ef38db206a4464b9dd83c4950cd8b8a0bc098))
* **pdf:** preserve native coverage with vector contours ([4cd29c9](https://github.com/twangodev/sdocx/commit/4cd29c9fd9321cf09bc7057c6f8d5b33eefc3feb))
* **pdf:** preserve supplied SVG color glyph paint ([c3adc12](https://github.com/twangodev/sdocx/commit/c3adc12a23bc02243a45c34ec5e08dfccacc53ea))
* **pdf:** retain continuous image bodies over imported paper ([67b08db](https://github.com/twangodev/sdocx/commit/67b08db6fe250a80368f1566e52a70a165563a64))
* **pdf:** support explicit native paper load axes ([d2c1407](https://github.com/twangodev/sdocx/commit/d2c1407a9742ec5e6434e3c0c0b8d2d169f07fb1))
* **pdf:** transport typed native ink fields as vector Forms ([e839374](https://github.com/twangodev/sdocx/commit/e839374c483ac7b9e4b21f56e4afa2ed6c173b59))
* **render:** add bounded vector arrowheads and tip cleanup ([381dbf1](https://github.com/twangodev/sdocx/commit/381dbf1febf57d117dfe7f34a68939a3a455fa69))
* **render:** add ChineseBrush atlas paint profiles ([6ae0cba](https://github.com/twangodev/sdocx/commit/6ae0cba20d292c99370a006d42bc82c967f15bc9))
* **render:** add explicit ChineseBrush fringe scale ([cf45beb](https://github.com/twangodev/sdocx/commit/cf45bebb7f6d30c1059c866f4fd58bccdc723880))
* **render:** normalize saved shape paths through native Model semantics ([1643c0b](https://github.com/twangodev/sdocx/commit/1643c0b5986ea68ad38d33db63379c54c58901f9))
* **render:** place Formula PNGs using native bounds ([dac32ff](https://github.com/twangodev/sdocx/commit/dac32ffb47a65ea718b236266d69f8241e2c6025))
* **render:** preserve Fountain V16 radial coverage as vectors ([ae4f96f](https://github.com/twangodev/sdocx/commit/ae4f96fcf45b9d44291d5803a105584e2e0e0a43))
* **render:** preserve native DefaultPen disk order ([0bf1549](https://github.com/twangodev/sdocx/commit/0bf1549e7c1a4866b91ece917201ed1926e3bf09))
* **render:** preserve native Formula stroke fallback ([c0034a8](https://github.com/twangodev/sdocx/commit/c0034a81a820eb0acfd60d60f4e5dc80f4263745))
* **render:** preserve native saved dash styles in vector outlines ([9e99097](https://github.com/twangodev/sdocx/commit/9e990972d30b3933a0839d777508e500d4637477))
* **render:** use bounded native Oblique V4 vector coverage ([d1819df](https://github.com/twangodev/sdocx/commit/d1819dfe84cdf4ca8b667be46c464d3fd74eadc3))
* **render:** use native Calligraphy meshes in vector exports ([7b2fbe4](https://github.com/twangodev/sdocx/commit/7b2fbe45a67261d876b1a186910512016f3150cb))


### Bug Fixes

* **decode:** match native compressed coordinate rounding ([144a462](https://github.com/twangodev/sdocx/commit/144a4624b783fc53e42963269d77ce6e59d2f90b))
* **decode:** match native float channel accumulation ([3feadc5](https://github.com/twangodev/sdocx/commit/3feadc59ac8bce6d928216f55a8cf1fb2a0e8440))
* **ink:** admit bounded native InkPen2 AA-off gradients ([a9b5557](https://github.com/twangodev/sdocx/commit/a9b55570c1db69e36e195fc164d35d290b175f94))
* **ink:** admit native Marker3 V5 strokes with curves disabled ([59e9d00](https://github.com/twangodev/sdocx/commit/59e9d003223ff42a4fd63342dbefd943f8f4a23b))
* **ink:** apply native Calligraphy style defaults ([14373e0](https://github.com/twangodev/sdocx/commit/14373e0d931f3498ca6f69626f3c31d8a4a9bd74))
* **ink:** apply native default black to InkPen V1 exports ([cafc899](https://github.com/twangodev/sdocx/commit/cafc8993ba4a2dee9c286d5ec2b83bc2e32afaae))
* **ink:** apply native Fountain V14 saved defaults ([3800690](https://github.com/twangodev/sdocx/commit/38006907b42c3ecb53c3891480b78ad378a19812))
* **ink:** apply native MagicPen setting and color defaults ([a21a48f](https://github.com/twangodev/sdocx/commit/a21a48f60a1228c3f89d6ede69872446e0c0d9f1))
* **ink:** apply native Oblique saved-style defaults ([f1006df](https://github.com/twangodev/sdocx/commit/f1006dfcea7884384aedf237c7e94f0ef3fb5fa3))
* **ink:** apply native omitted type defaults to parsed InkPen2 strokes ([ac451e4](https://github.com/twangodev/sdocx/commit/ac451e4bdb5ad6f9bf14195d9165e4a3998462af))
* **ink:** apply saved InkPen alpha once per stroke ([6713fbf](https://github.com/twangodev/sdocx/commit/6713fbf4f4dc9fce5bcc4194437367f3351f7e37))
* **ink:** center Marker4 V8 stamps on native positions ([9f224a2](https://github.com/twangodev/sdocx/commit/9f224a2d705312961a1b82cb1dac83d2b885346b))
* **ink:** clamp native Pencil3 terminal particle alpha ([9c0c03c](https://github.com/twangodev/sdocx/commit/9c0c03c1d4318e826b3216a8ea911b2ab9de2de2))
* **ink:** clamp Oblique size through native pen settings ([f31d224](https://github.com/twangodev/sdocx/commit/f31d2249ad7da936faa0a8339d3ad8ef6aa053de))
* **ink:** composite Fountain V16 stamps with native source-over alpha ([1db7f49](https://github.com/twangodev/sdocx/commit/1db7f49e6e3837d3ab382e35117d5535dff506f9))
* **ink:** distinguish Marker3 V1 and V5 caller settings ([9baaf6e](https://github.com/twangodev/sdocx/commit/9baaf6e8176f94e6188aea6a6f43a781e6254ec7))
* **ink:** honor omitted native MagicPen style defaults ([ba2868b](https://github.com/twangodev/sdocx/commit/ba2868b76b78d85ecafddd63fe3065b21811645e))
* **ink:** honor saved Fountain V16 size and endpoint timing ([7cba51a](https://github.com/twangodev/sdocx/commit/7cba51a783fa451bc9726a23a93bf7f6c228ba9a))
* **ink:** index MagicPen replay prefixes by prepared stamps ([3066cea](https://github.com/twangodev/sdocx/commit/3066cea2fb63f720aaaf28c7a943200ff7363d3f))
* **ink:** isolate staged gradient MAX mask inputs ([185baca](https://github.com/twangodev/sdocx/commit/185baca82ab1b8069c6870001360e42c7c838ebb))
* **ink:** keep native InkPen2 color effects outside solid-color reconstruction ([b92f1e0](https://github.com/twangodev/sdocx/commit/b92f1e0f5cb71fc68fc72c1a4237fe4215b38f97))
* **ink:** match ChineseBrush NoCurve terminal sampling ([452a396](https://github.com/twangodev/sdocx/commit/452a3963a054e14f6527e314a3a8123c7d111761))
* **ink:** match native Calligraphy constructor state ([b05b03e](https://github.com/twangodev/sdocx/commit/b05b03eba87027274092cbaa7b27017386f3f6b1))
* **ink:** match native InkPen size defaults and tools ([da181c0](https://github.com/twangodev/sdocx/commit/da181c0e7d1cbe151c2683c7db4f97e52b8306e6))
* **ink:** match native MagicPen terminal pressure and rejected tails ([22bdc7d](https://github.com/twangodev/sdocx/commit/22bdc7d6e691b91735992996e0cb9cdad5ffebc9))
* **ink:** match native MagicPen thick-stroke sampling ([b827f4a](https://github.com/twangodev/sdocx/commit/b827f4a9f18c386f7a57bcc49c8edbf2315b1848))
* **ink:** match native Oblique settings version resolution ([4d3c936](https://github.com/twangodev/sdocx/commit/4d3c936720b9babe196ba81561b281e6294ef6e8))
* **ink:** match native Oblique zero-length traversal ([7106f21](https://github.com/twangodev/sdocx/commit/7106f215976b858e149a844f82f53aaf6a9a2e81))
* **ink:** preserve ChineseBrush raw terminal events ([8c84636](https://github.com/twangodev/sdocx/commit/8c846360d26026b4b8bfdb17974019221186d309))
* **ink:** preserve MagicPen small-stamp vector coverage ([08a3ba4](https://github.com/twangodev/sdocx/commit/08a3ba46a5672f96842633788767dd52abf0d94f))
* **ink:** preserve native Calligraphy state across saved strokes ([bf97a8e](https://github.com/twangodev/sdocx/commit/bf97a8e19c59fbe070dad129fe5149c664fa46fd))
* **ink:** preserve native ChineseBrush default colors ([d01d68c](https://github.com/twangodev/sdocx/commit/d01d68c85afb2bcc7684d4358dd2e160bf3fe228))
* **ink:** preserve native vectors when gradient effects are disabled ([7f854a3](https://github.com/twangodev/sdocx/commit/7f854a3adfae7e88a1d5500abfc1f0694fbcc0c7))
* **ink:** preserve Oblique cursor for short saved strokes ([5ce959d](https://github.com/twangodev/sdocx/commit/5ce959d820702290136dfa785f94905f3a06368f))
* **ink:** reconstruct short and duplicate Oblique V1 paths ([f4d92df](https://github.com/twangodev/sdocx/commit/f4d92df7f2ebca949f30378f4e17e7f2fc1cd529))
* **ink:** recover MagicPen curved width and direct-dot contracts ([d9330b3](https://github.com/twangodev/sdocx/commit/d9330b354ece910bc28b4338d8ce7aa052208362))
* **ink:** recover saved InkPen RGB when public color is absent ([36aef2a](https://github.com/twangodev/sdocx/commit/36aef2aa4ac13ab10db7002d5613d5acbe663bd5))
* **ink:** reject singleton sources in saved-page rendering ([974186e](https://github.com/twangodev/sdocx/commit/974186e86d0ea0f312996e7e2735a4b260962bcb))
* **ink:** reproduce native Calligraphy fixed-width extents ([59c9109](https://github.com/twangodev/sdocx/commit/59c9109a922cf65fdb6e013abe29aaed17e248c9))
* **ink:** retain InkPen2 state across unstamped segments ([b970343](https://github.com/twangodev/sdocx/commit/b97034381acc1244057c9eb16111cd612b446daf))
* **ink:** retain native AirBrush source RGB defaults ([f1a790f](https://github.com/twangodev/sdocx/commit/f1a790f05011026a774bde5d8cb7253b805c570b))
* **ink:** retain native InkPen2 vectors for inert pattern mode ([8773206](https://github.com/twangodev/sdocx/commit/8773206167e8a65624a5f9a3f98ee8d27eab83af))
* **ink:** use native Marker3 V5 stamp geometry for page exports ([d5f0d1a](https://github.com/twangodev/sdocx/commit/d5f0d1ab90d796b0f21c30e0b170f71c0484176b))
* **ink:** use saved Marker4 sizes and StraightHighlighter alias ([133cc14](https://github.com/twangodev/sdocx/commit/133cc14788e7f9ce1c56f5e72542ddabae0b50bf))
* **pdf:** bound ink tiling without rounding remainders ([9204845](https://github.com/twangodev/sdocx/commit/92048451581a01f3458a1919d74bb73f2fb1d7ea))
* **pdf:** preserve logical text across nested replacements ([6b838f0](https://github.com/twangodev/sdocx/commit/6b838f0507a2bc00712ea77350bac93c671a492d))
* **pdf:** preserve small contours beside large fields ([d979abe](https://github.com/twangodev/sdocx/commit/d979abef34b50dc33374362e2ab64d3c08c85902))
* **pdf:** reject exports that omit retained PDF paper ([d78c18b](https://github.com/twangodev/sdocx/commit/d78c18b239d6a5425f328b882a914b91a7cd5b88))
* **pdf:** reject shifted colorizer backdrops ([4976828](https://github.com/twangodev/sdocx/commit/49768288f4e25a091f32bf2b9ffc9e83a99c4c40))
* **pdf:** retain complete continuous-page body source ([5131eaa](https://github.com/twangodev/sdocx/commit/5131eaa92b1f05521892b0346c69966fbd5a2fe3))
* **render:** apply native DefaultPen size and tool defaults ([0ffd2b1](https://github.com/twangodev/sdocx/commit/0ffd2b1447a141894444a1f3842a379375d774b3))
* **render:** map source-pixel crops without original placement ([bc31492](https://github.com/twangodev/sdocx/commit/bc31492eff6d9bce150171c4cebfef1c74fe11ee))
* **render:** preserve formula image type for drawing batches ([39b2a5c](https://github.com/twangodev/sdocx/commit/39b2a5c246498bbba2447f5620c45c85fe156031))
* **render:** preserve native Fountain missing-color default ([01a0fb0](https://github.com/twangodev/sdocx/commit/01a0fb01e7cdeadb6357172f36a90caa77612f5f))
* **render:** preserve native Marker2 zero-width strokes ([eda0c9a](https://github.com/twangodev/sdocx/commit/eda0c9a81e013562af7a9d055d09eafdbeb48cef))
* **render:** reject collapsed Calligraphy mesh transforms ([8583102](https://github.com/twangodev/sdocx/commit/85831026a3ad1acc9a08bc2410357d469010bb22))
* **render:** report omitted imported PDF backgrounds ([e54afb3](https://github.com/twangodev/sdocx/commit/e54afb3d1bc68a7817e320875c646420af2856c0))
* **render:** restore Fountain V14 ink in Firefox ([0aa4e24](https://github.com/twangodev/sdocx/commit/0aa4e2468abd46a470790def7806b6c48d8dd96a))
* **render:** retain DefaultPen geometry without explicit color ([78e2656](https://github.com/twangodev/sdocx/commit/78e2656317ced17eed05e97015a9c9d1130d8c5e))
* **render:** retain full continuous body in SVG and replay ([f218f26](https://github.com/twangodev/sdocx/commit/f218f2681236108975b3ce6b4b5ce85e0b7ec585))
* **render:** stabilize Calligraphy gradient coordinates ([15c8164](https://github.com/twangodev/sdocx/commit/15c81643227804bdfb84fabea49ea0a8987ef619))
* **shape:** stabilize arc math across host platforms ([c08b600](https://github.com/twangodev/sdocx/commit/c08b600d7d1257a29b85329c471bcc7e01f99066))
* **text:** match native body and cell bullet sizing ([a1a4e6e](https://github.com/twangodev/sdocx/commit/a1a4e6ea2e0182ca02e8e4907942e9246b340b04))
* **text:** preserve italic combining glyph origins in SVG ([b9ab253](https://github.com/twangodev/sdocx/commit/b9ab253b57f05ab6764cbcf4bb6d107195beef72))
* **text:** preserve native Flow italic shear in SVG ([4f19db4](https://github.com/twangodev/sdocx/commit/4f19db419d504bf7e7c35d151c38597ffa98ff2f))
* **text:** preserve native italic shear across mixed fragments ([926daf7](https://github.com/twangodev/sdocx/commit/926daf7b861c8f7ce8c5c2e88131d16a72600631))
* **text:** retain markers in empty bullet paragraphs ([ee85bcb](https://github.com/twangodev/sdocx/commit/ee85bcbef1dd28255141771b9db84b688f4ed473))
* **text:** select supplied CJK fallback fonts per grapheme ([739d3ad](https://github.com/twangodev/sdocx/commit/739d3ad0f11846843f26b32641baaec4ada77899))
* **text:** synthesize flow italics from native regular faces ([f099d80](https://github.com/twangodev/sdocx/commit/f099d801a190d9ac928021b95767659257b713e7))


### Performance Improvements

* **ink:** skip unused full-page Fountain culling bounds ([2f39650](https://github.com/twangodev/sdocx/commit/2f396503d961b521230262af416812adf8b6e957))
* **pdf:** compress final paper object dictionaries ([caed915](https://github.com/twangodev/sdocx/commit/caed915491d764b47aded0ef63381814c52b0082))
* **pdf:** index native ink markers in one pass ([10260e3](https://github.com/twangodev/sdocx/commit/10260e3a0f1d5a6335ebb362e78d120078d31a74))
* **pdf:** skip paint outside the SVG canvas ([4d74a89](https://github.com/twangodev/sdocx/commit/4d74a89867ca67ca065b54c0e91fb8d9803b889f))
* **render:** share ChineseBrush atlas images ([ec2ccd3](https://github.com/twangodev/sdocx/commit/ec2ccd32a3bffb3b490f15cb9fa40417ace9abb9))

## [1.1.0](https://github.com/twangodev/sdocx/compare/sdocx-v1.0.0...sdocx-v1.1.0) (2026-10-09)


### Features

* report native decode and vector processing progress ([a79443d](https://github.com/twangodev/sdocx/commit/a79443d0cf8c3984dc1bba41742688dcab023be1))

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
