# Web, Link and attached-file card sources

## Evidence boundary

These are static Java and ARM64 traces in Samsung Notes 4.4.45.37. No native execution,
generated document, archive round trip or paired appearance comparison was performed.
Addresses are ELF virtual addresses, qualified by Model, Drawing or Composer below. Decompiled
Java paths are relative to
`scratch/apk-analysis-decompiled/sources/com/samsung/android/support/senl/nt/`.

The APK is
`com.samsung.android.app.notes_4.4.45.37-444537000_minAPI29(arm64-v8a,armeabi-v7a)(nodpi)_apkmirror.com.apk`,
SHA-256 `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.

| APK ARM64 library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |

## Own frames retain source metadata independently of appearance

The following are type-specific own-frame fields, separate from common [ObjectBase flexible
metadata](object-flexible-findings.md). Strings below use a u16 UTF-16-unit count followed by
those units; gradients are u32-sized `BackgroundGradientColor` own frames. These serializers
do not save measured card glyph/layout paths. Readers use `ReadString2` and only replace
decoded strings when nonempty, so empty-field decoding does not establish clearing an already
populated object.

Web type 13 has a 15-byte own-frame header and flexible data only: Model
`ObjectWebImpl::GetOwnBinary` `0x4409a4` calls writer `0x43f6f0`; reader `ApplyOwnBinary`
`0x440a58` checks type 13 and calls `0x43fcec` (`0x440af0`, `0x440ba0`).

| Flexible bit | Stored field | Model writer evidence |
| --- | --- | --- |
| 0 | i32 attached HTML media ID, omitted at -1 | `0x43f728–0x43f780` |
| 1 | i32 thumbnail media ID, omitted at -1 | `0x43f784–0x43f7dc` |
| 2, 3, 4 | body, title, URI strings, in that order | `0x43f7e0–0x43f914` |
| 5 | i32 version, always emitted | `0x43f960–0x43f9ac` |
| 6 | i32 view type, omitted at 0 | `0x43f9b0–0x43fa04` |
| 7 | sized gradient frame | `0x43fa44–0x43fac8` |
| 9 | two u32 dummy-thumbnail colors | `0x43facc–0x43fb74` |
| 10 | i32 state, omitted at 0 | `0x43fb78–0x43fbcc` |

One mask-independent i32 image-type ID is always written after bits 0–4 and before version
(`0x43f918–0x43f95c`), and always read (`0x43fe4c–0x43fe68`). Direct getters establish image
type at impl+264 (`0x43dd6c`), version at +268 (`0x43e47c`), view type at +272 (`0x43e730`)
and state at +276 (`0x43e1dc`). No bit-8 payload is emitted or consumed by these routines.

Link type 17 has fixed data followed by flexible data. Model own writer `0x44c504` calls fixed
writer `0x44c5f8`, then flexible writer `0x44c8c8`. Own reader `0x44cafc` calls matching
readers `0x44ccec` / `0x44cf24` (`0x44cc30`, `0x44cc58`). Fixed order is package name, action,
i32 thumbnail media ID (including -1), title, body, option, i32 version and i32 view type
(`0x44c610–0x44c8b0`). The five named text fields are strings. Package/action getters
`0x44a5e4` / `0x44a750` and title/body/option getters `0x44aa44` / `0x44abb0` / `0x44ad1c`
confirm their identities. Flexible bit 0 is i32 icon type, omitted at 0 (`0x44c900–0x44c954`);
bit 1 is a sized gradient (`0x44c958–0x44ca1c`); bit 3 is two u32 dummy-thumbnail colors
(`0x44ca20–0x44cac8`). No bit-2 payload is emitted or consumed by these routines.

AttachedFile type 24 has a 15-byte own header and flexible data only: Model own writer
`0x460f5c` calls `0x46008c`; own reader `0x461010` checks type 24 and calls `0x46053c`.

| Flexible bit | Stored field | Model writer evidence |
| --- | --- | --- |
| 0 | i32 source attachment media ID, omitted at -1 | `0x4600c4–0x46011c` |
| 1 | title string | `0x460120–0x460184` |
| 2 | sized gradient frame | `0x460188–0x46024c` |
| 3 | i32 thumbnail media ID, omitted at -1 | `0x460250–0x4602a8` |
| 4 | body string | `0x4602ac–0x460310` |
| 5 | u32 tint then u32 background color | `0x460314–0x4603bc` |
| 6 | i32 view type, omitted at 0 | `0x4603c0–0x460414` |

Its runtime state at impl+144 is not an own-frame field; successful load sets it to 4
(`0x461160–0x461164`). Absent bit 6 resets view type to 0 (`0x4606a0–0x4606a4`). Title/body
are card strings, not typed MIME or file-size fields. Manager size metadata is separate from
this own payload.

## Bound resources have consumers beyond card drawing

Web has independent FileAttachers at impl+16 (HTML) and +128 (thumbnail); Link has one at +64
(thumbnail); AttachedFile has two at +32 (source) and +192 (thumbnail). FileAttacher IDs at
+72 account for the saved reference members: Web +88/+200, Link +136, AttachedFile +104/+264.
Web resource enumeration appends separate HTML and thumbnail hash/path pairs
(`0x43ed00–0x43ed1c`, `0x43edc4–0x43ede0`); Link appends its thumbnail pair
(`0x44bcc8–0x44bd40`). The own frames contain IDs, not bound file bytes or saved paths.

Model `FileAttacher::bindFile` `0x2b46d4` stores a manager-returned ID or a detached path;
`GetFilePath` `0x2b487c` resolves registered IDs. Vtable relocations `0x491a60`, `0x491a68`,
`0x491a90` identify manager Bind(path) `0x28c5dc`, Bind(int) `0x28e838` and GetFilePathById
`0x28f98c`. Archive inclusion and original-byte preservation remain subject to the [media save
and binding gates](vector-retention-findings.md).

The app sets Web body from `CharUtils.fromHtml(description).toString()`, title, thumbnail, URI
and optional scrapbook attachment separately
(`composer/reflect/DocumentContractImpl.java:196–211`). Opening reads URI and attachment path
and chooses browser intent or ScrapBookViewer
(`composer/main/base/presenter/task/TaskOpenWebCard.java:64–69`). The latter calls
`MhtmlExtractor(...).extract(uri, cacheDirectory)`
(`composer/main/base/external/scrapbook/ScrapBookViewer.java:85–95`;
`MhtmlExtractor.java:439–461`). This is an MHTML-oriented attachment consumer, not evidence
that HTML-named bound bytes are plain HTML or saved card vectors.

Link click handling sets the Intent package and parses saved action JSON for `key_action`,
URI/MIME data and extras; an image MIME branch can use option as a file path
(`composer/main/base/presenter/composer/listener/ControlObjectListenerImpl.java:94–119,203–205`).
Its text-sharing helper chooses option only when it starts with http, otherwise title
(`composer/main/base/presenter/share/HandleLinkCard.java:13–18`). Option is therefore not an
unconditional URL alias, and preview text omits click data.

AttachedFile extraction reads the resolved source path, derives its extension, uses card title
for the destination name and copies that file
(`composer/main/base/presenter/task/TaskSaveFileTo.java:85–107`, copy at 100). This consumer
saves source bytes independently of the displayed card. It does not establish byte-for-byte
preservation through all binding/import routes.

## Drawing and PDF export rebuild card appearance

Drawing Web reads saved title/body/URI (`0x9908c`, `0x99164`, `0x99248`), can shorten URI
(`0x9925c`), and produces measured/ellipsized SimpleTextView text in `getWebText` `0x9872c`.
`GetWebUriData` `0x9895c` returns displayed text plus a separate copy of the complete view
text (`0x98980–0x9899c`), which may already contain a shortened URI. Link `measureText`
`0x87134` similarly reads title/body/option and may shorten option (`0x87168`, `0x87224`,
`0x87308`, `0x87374`); `getData` `0x880f0` emits prepared ellipsized text. AttachedFile
title/body/path helpers read model title, body and resolved source path (`0x7d9cc`, `0x7daa4`,
`0x7db7c`); prepared path text can be ellipsized by `getAttachedFileText` `0x7d128`
(`0x7d1ac–0x7d1d0`). These runtime layout results are distinct from the own-frame strings and
resource IDs.

Web `drawThumbnail` `0x98f68` uses CardDrawing's bitmap producer `0x70ac0`, which decodes an
admitted image path or draws a resource VectorDrawable into a bitmap (`0x70af8–0x70b38`,
`0x70b44–0x70c88`). The fallback is generated resource artwork, not a saved object-specific
path. Link separately decodes and rounds its thumbnail (`0x87748`, `0x877c0`, `0x877e4`).
AttachedFile's thumbnail getter is independent of its source getter (`0x7dbe4`). These preview
routes do not turn bound source documents into card geometry.

Composer PDF exporters construct and measure Drawing objects, then export background, title,
body, URI/option/path text and thumbnail separately: Web `0x354058` (`0x354170–0x3541b0`),
Link `0x34ccb4` (`0x34cdcc–0x34ce0c`), AttachedFile `0x34b9d8` (`0x34baf8–0x34bb38`). Text
writers populate PdfTextAdapter from DrawnText via SetTextCommonInfo (`0x354c48`, `0x34d878`,
`0x34c594`). Thumbnail writers instead supply rounded bitmaps to PdfImageAdapter (`0x354a7c`,
`0x34d6ac`, `0x34c3c8`). AttachedFile background uses rounded PDF paths (`0x34bcbc`,
`0x34bce4`, `0x34bdbc`); its separate PDFWriter also writes prepared card text and
thumbnail bitmap (`0x377aec`, `0x3782b8`). These are card path/text/image routes, not an
established exporter for the attached document's internal vectors or MHTML contents. No
executed PDF appearance, full source embedding or SVG parity follows from these traces.

## Current Rust ownership

`types.rs` recognizes Web=13, Link=17 and AttachedFile=24, but `page.rs::decode_objects`
produces no dedicated high-level node: visible cards receive UnsupportedObjectType
(`page.rs:185–196`), supported children are traversed separately (`198–232`), and hidden
recognized records skip earlier (`85–88`). Card strings, action metadata and resource bindings
are not projected.

The [raw-page ownership boundary](vector-retention-findings.md) still applies:
`StoredObject::payload` requires supplied original page bytes (`storage.rs:215–218`).
Non-directory `media/` files, including HTML/MHTML and attached files present there,
are retained as opaque sources alongside typed image assets. This does not project
card metadata or render cards; files outside `media/` and raw card payloads still require
the original input. Saved source metadata, measured display text and preview pixels are distinct.
