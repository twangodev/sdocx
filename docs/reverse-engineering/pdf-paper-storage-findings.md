# PDF paper: saved records and native page state

The evidence is Samsung Notes APK 4.4.45.37, SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`,
its decompiled Java, static arm64 instructions, and the scoped predicate capture
linked below. Java paths are relative to `sources/`; addresses
below belong to `libSPenWDoc.so`, SHA-256
`1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6`.
Loader, setter and serialization findings remain static; no real-document PDF
capture is asserted here.
The [page background findings](page-background-findings.md) give the wire field
table, rectangle version split, template identifiers and drawing caller map.

## Saved presence is distinct from native PDF availability

The Java binary reader (`n1/u.java:350–373`) clears and replaces the PDF list
only when flexible field bit 8 is present. A present count of zero clears it;
an absent field leaves the existing list untouched in that reader method. The
template-type field similarly assigns its member only when bit 9 is present.
The current writer emits bit 8 only for a nonempty list (`1038–1053`), and bit 9
only for a nonzero type (`1054–1058`). Omission is therefore not an explicit
clear instruction in every consumer.

`WPageLoadHandler::LoadHeader_PdfInfo` (`0xd4584`) has the same present-versus-
absent list distinction. It tests bit 8, clears existing native records when
present, and reads a zero-extended 16-bit count. The Java count is also unsigned:
`f2/a.java:138–140` returns 0–65535 without the signed-short cast used by its
UTF-16 string reader.

The native loader does more than decode the wire record. For each record it:

- Reads the binding ID and page index as 32-bit words, then reads four integer
  rectangle coordinates and converts them to runtime `f32` (`0xd4630–0xd468c`).
- Requires a nonnegative binding ID before calling
  `MediaFileManager::CheckBoundFileInfoValidity` with the recovery flag.
- Resolves the file path by that binding and appends the record only when the
  binding check succeeds (`0xd4690–0xd46dc`).
- Skips negative binding IDs. The special value `-1000` sets the page's external
  PDF flag instead of appending an ordinary record (`0xd46e0–0xd470c`).
- On a failed nonnegative binding, returns error 24 in ordinary mode; recovery
  mode records a damaged-data flag and skips the entry (`0xd4710–0xd4828`).

No page-index range validation appears in this loader. Validation of resource
existence is a separate operation from validating a particular page inside it.
The native routine always reads integer rectangles; its second argument is a
field mask, not a format version. Its caller `WPageLoadHandler::LoadHeader`
(`0xd234c`) admits only unsigned format versions greater than 2033 and rejects
older versions with error 13 (`0xd2380`). The Java reader's older float branch
is a different compatibility boundary, not behavior of this native loader.

The current native writer, `WPageSaveHandler::Save_PdfInfo` (`0xd6468`), emits
binding ID, page index and four integer rectangle coordinates. At `0xd6588`,
`fcvtzs v0.4s` converts the runtime floats by truncating toward zero for finite
in-range values. It omits bit 8 when the native list is empty. Binding `-1000`
bypasses ordinary media validity checking (`0xd6528–0xd654c`). This describes
ordinary valid-record output: the recovery branch can skip invalid records
without backpatching the originally written count. It is not evidence of a
well-formed recovered list. Paths and passwords are not fields written beside
these coordinates.

## Runtime PDF data is richer than the saved record

The constructor at `0xc3270`, native copies at `0xce848`, and JNI conversion at
`0xe2078` establish this 72-byte arm64 representation:

| Offset | Native runtime member | Constructor state |
| ---: | --- | --- |
| `0x00` | `String` file path | Empty native string |
| `0x10` | Signed binding ID | `-1` |
| `0x14` | Signed PDF page index | `0` |
| `0x18` | Four `f32` rectangle coordinates | Zero rectangle |
| `0x28` | `String` password | Empty native string |
| `0x38` | Additional internal string | Empty; not marshalled as a Java PDFData field |

`JNI_PDFDataList::GetCPDFData` (`0xe2078`) reads the Java fields `filePath`,
`index`, `rect`, `boundFileId` and `password`. The index is copied directly as
`jint`; a nonnull rectangle is copied as four floats. Null path, rectangle or
password leaves the native constructor default for that member. No finite,
rectangle-order or page-index bounds check occurs in this conversion. A null
record object itself takes an error path.

The list converter (`0xe25ec`) skips an element whose record conversion returns
null; it does not necessarily reject the entire list. `WPage_SetPDFData`
(`0xf84c8`) rejects a null Java list itself with error 7. `WPage_GetPDFData`
(`0xf86a0`) uses `ConvertToJPDFDataList` (`0xe24b4`), which constructs an empty
Java `ArrayList` even when a valid native page has no PDF records.

`GetJPDFData` (`0xe1dd0`) marshals those same members back into a Java object.
The Java declaration exposes mutable index/rectangle/password; path/binding are
private final (`com/samsung/android/sdk/pen/worddoc/SpenWPage.java:458–471`).

The native page holds the ordered list at implementation offset `0x178`.
`WPage::GetPDFData` (`0xc61d0`) returns that list's address internally; Java's
getter goes through JNI conversion rather than exposing that native pointer.
`WPage::HasPDF` (`0xc6228`; implementation `0xcec14`) tests whether the list count
is positive. It does not check template type, file existence or page readability.
The overload `HasPDF(int)` (`0xc628c`) compares **binding IDs** at record offset
`0x10`, not PDF page indices at offset `0x14`.

[Captured predicates](../../conformance/pdf-paper-predicates.json) execute
unchanged WPage getters/predicates and actual BaseArrayList operations on supplied
WPage facade/implementation and 72-byte records. They confirm binding matching,
including negative queries; they do not construct WPage/PDFData or exercise
setters, loaders, serialization, resource checks or rendering.
These query results are not resource-admission rules.

## Public setter, history setter and template setter differ

`SpenWPage::setPdfData` rejects a null Java list and delegates nonnull lists to
the native setter (`SpenWPage.java:1133–1144`). It does not reject an empty list.

The native public `WPage::SetPDFData(ArrayList*)` (`0xc5f4c`) enters
`m_SetPDFData` (`0xc2d78`). A null/empty input is a successful no-op when the old
list is empty; otherwise it proceeds to replacement and clears the old records.
The distinct history overload at `0xc5f54` returns success immediately for null
input. Null therefore has no single meaning across these native interfaces.

`WPageImpl::SetPDFData` (`0xce670`) clears its existing list first, then makes
ordered copies of nonempty input records and refreshes media bindings. Copies
retain path, binding, index, rectangle and password; the additional internal
string is constructed empty (`0xce848`). This path does not validate page index
or rectangle geometry. Its empty-input branch returns before the nonempty
changed/coedit notification path.

`SetPageTemplateInfo` (`0xcbde4`) calls the implementation setter with the PDF
list from the template-info structure. A null list there clears PDFs. This route
also sets color, type, URI and image mode independently; selecting template type
alone is not synonymous with supplying PDF records.

`BindReleasePDF` (`0xce14c`) binds IDs and fills paths through the media manager.
On release, sentinel `-1000` skips only the manager's `Release` call; every record
still resets its binding ID to `-1` and clears its path (`0xce1dc–0xce1fc`).
`ReferExternalPDF` (`0xceb10`) releases current
bindings, writes `-1000`, clears paths and sets the external flag. `ClearPDFData`
(`0xcd7d8`) releases bindings, destroys records and clears that flag.
`IsPDFAvailable` (`0xcad7c`) delegates to the manager's file-existence check;
it is distinct from the list-count predicate. History packing (`0xce794`) stores
count, binding ID, index and float rectangle, not path/password. Unpacking
(`0xcdfd8`) reconstructs these values and resolves paths through the manager.

## Source identity and placement are separate

The compact Java saved-data holder `n1/t.java:5–9` has an index, nullable `RectF`,
and binding ID initialized to `-1`. There is no password/path member in this
holder. In synchronization XML, `n1/u.java:237–248` writes `pageIndex`, a media
hash obtained from the binding manager, and integer `pdfRect`. The XML reader
maps `hash` back to a binding ID, parses signed decimal indices, accepts integer
`pdfRect`, and accepts legacy float `rect` when the rectangle is still null
(`719–752`; integer/float helpers in `a1/t.java:485–566`). The synchronization
identity is thus a media hash rather than a pathname copied into each page.

Saved coordinates are page placement values, not intrinsically PDF points.
The native page-load path at `0xd3760` can scale all PDF rectangles by the ratio
of target page size to stored page size (`0xd38dc–0xd38fc`). The saved integers,
their runtime `f32` conversion, and post-load scaled rectangles are distinct
numeric states. The wire reader/writer impose no rectangle ordering or clipping
normalization in the paths inspected here.

Application callers also distinguish imported PDF content from a PDF template:
`PositionUtil::adjustPositionWithPdfRect` ignores template type 16 but positions
new content after PDF placement when `hasPDF()` is true with another template
type (`com/samsung/android/support/senl/nt/model/base/utils/PositionUtil.java:23–29`).
`NoteManager::addPages` copies the PDF record list only for type-16 pages
(`com/samsung/android/support/senl/nt/composer/main/base/model/composer/NoteManager.java:125–155`).
The presence of PDF records cannot safely be used to invent a type-16 assignment.

## Current Rust interpretation

[`page.rs`](../../crates/sdocx/src/page.rs) consumes all saved PDF entries but
retains only the first index as unsigned `u32`, overriding the template source
with `CustomPdf`. Thus a native negative index retains its bits but is exposed
as a large positive value; source binding, placement and additional indices are
not exposed. The override also conflates PDF presence with template backing,
although native `HasPDF` and type 16 are distinct above.

[`container.rs`](../../crates/sdocx/src/container.rs) loads only JPEG/PNG/WebP
entries into `DocumentMetadata.media_assets`; PDF bytes are omitted.
[PDF resource resolution](pdf-paper-resource-findings.md) describes the separate
manifest and open-file contracts. Neither
[`StoredPage`](../../crates/sdocx/src/storage.rs) nor the high-level page owns a
typed PDF record list. Stored offsets require the caller's original page bytes
to revisit the discarded fields. The existing
[`pdf_template_index_follows_the_declared_record_instead_of_page_size`](../../crates/sdocx/tests/structural_strokes.rs)
test establishes only first-index selection from supplied bytes, not media
binding, placement, native load filtering or PDF rendering.
