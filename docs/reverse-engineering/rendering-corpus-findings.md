# Rendering corpus findings

This inventory uses the Rust SDK at `77ac260` and the available source archives,
without generating SVGs. Parser acceptance, stored feature occurrence and native
appearance equivalence are separate observations. An empty parse report does not
establish complete rendering support.

## Sources and identity

The four paired Samsung Notes fixtures in `hf` are locked by
[`conformance/corpus.json`](../../conformance/corpus.json). The checked-out corpus
revision is `0865fd317776fa4fd60388e93724f789476541d4`; all eight source/PDF hashes
match the manifest. Its [dataset description](../../hf/README.md) records the
fixture contents, attribution and limitations.

Three additional source documents are available locally under
`tmp/stroke-conformance`. Their hashes match the earlier
[fountain comparison](fountain-parity.md#real-document-geometry-audit). They are not
registered in the locked corpus and have no paired reference PDF in that corpus.
Their authoring device, application version and redistribution provenance are
not established by this inventory. The embedded PDFs in two archives are source
page assets, not independent Samsung Notes exports.

| Source archive | SHA-256 |
| --- | --- |
| `01-basic-formatting.sdocx` | `86716d1ccc070d101272ff0760276b8e2d913bfcde88e34d3d64c5c51a1fd21b` |
| `02-shapes-and-dot-calibration.sdocx` | `dd6580a0531cf1712ef6526f1b0d51c42adaeb0d11feacca5d23ba562239da82` |
| `03-image-placement.sdocx` | `8bfa5a5117ab2d11a28058dee0f8fc61fdeaeb40a454af971e3bfb9a0ab7af9a` |
| `04-marker4-highlighter.sdocx` | `8299abbf763e8e3e5af746815d13dfdf45443da27b0eb79192c328571905cdd2` |
| `quiz.sdocx` | `38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a` |
| `cs61bl_su22.sdocx` | `fa2d3ba44023871c6a53436e810772f7b4f45b190dd28e05c172886b8f7e40a0` |
| `handwritten.sdocx` | `77e3997a066afa0333d0f5020bb428efffeb783c741bc956ae596292e2d5cda3` |

The three archives under `scratch/image-warning-corpus` and
`scratch/object-visibility-corpus` are byte-identical copies of fixtures 01 and
03, not additional evidence. Generated test and preview archives are excluded
from the real-document counts below.

## Observed structure

Physical counts include every stored layer and nested object record. Embedded
objects are counted separately from the flowing body in `note.note`.

| Document | Set | Format | Stored pages | Physical objects | Embedded objects | Parse diagnostics |
| --- | --- | ---: | ---: | --- | --- | --- |
| 01 | Locked | 5400 | 6 | None | 1 table, 1 code block | 0 |
| 02 | Locked | 4000 | 2 | 77 strokes, 5 shapes, 1 line | None | 0 |
| 03 | Locked | 4000 | 4 | None | 7 images | 0 |
| 04 | Locked | 4000 | 2 | 41 strokes | None | 0 |
| `quiz` | Local research | 4000 | 1 | 3,228 strokes | 1 image | 1 `UnsupportedPageTemplate` |
| `cs61bl_su22` | Local research | 4000 | 1 | 1,185 strokes | None | 1 `UnsupportedPageTemplate` |
| `handwritten` | Local research | 4000 | 1 | 2,769 strokes | None | 0 |

These seven inputs contain 7,300 strokes, 5 shapes, 1 line and 10 embedded
objects. No physical containers, standalone text boxes, unknown object IDs,
formula, math, plot, web, painting, video or attached-file objects occur.

Both template diagnostics refer to `CustomPdf { page_index: 0 }`. `quiz` retains
`media/0@pdf_1692322553368.pdf` (355,515 bytes); `cs61bl_su22` retains
`media/0@pdf_1691716745867.pdf` (572,913 bytes). The built-in templates observed
in the locked corpus are ID 7 in fixture 02 and ID 1 in fixture 04. No image-ID
or URI-backed background, nonzero background rotation, or other built-in
template ID occurs.

## Drawing features present and absent

| Saved pen settings | Strokes | Documents |
| --- | ---: | --- |
| FountainPen `14;` | 7,103 | `quiz`, `cs61bl_su22`, `handwritten` |
| FountainPen `18;0;100;` | 179 | 02, 04, `cs61bl_su22` |
| Marker4 `7;` | 6 | `quiz` |
| Marker4 `8;` | 12 | 04 |

All 7,300 strokes retain nonempty pressure, tilt and orientation channels. No
other pen profile occurs; the absence of another profile cannot certify its
decoder or fallback appearance. None sets the eraser, rainbow, straighten,
fixed-width or replay-only stroke property. The native geometry checks for these
fountain settings are recorded separately in [fountain parity](fountain-parity.md).

Fixture 02's shape IDs are 4, 1, 2, 11 and 6. All five shapes and its straight
line have stored paths, a solid black outline of width `9.116800308227539`, zero
dash/compound settings and zero begin/end arrow settings. Shape fills are absent;
their embedded text is empty. No gradients, connectors, nonzero geometry
rotation, arrowheads or advanced outline styles occur in these source documents.

Fixture 03 contains seven placements of one resolved PNG: ordinary scaling,
rotations of 180, 90, -90 and 45 degrees, and one rectangular crop
`[73, 50, 405, 222]` with an original placement rectangle. The additional image
in `quiz` uses media ID 7 without crop or rotation. No nine-patch, custom-mask,
image-border or alternate-original-image case is demonstrated by these inputs.

Fixture 01 supplies paragraphs, headings, lists, inline styles, six hyperlink
spans, one table and one code block. It does not supply a real-document example
of rotated/nested tables or general split-page table composition. Its Greek,
Cyrillic, CJK and emoji samples do not establish Samsung device font-fallback
parity. See [text vector support](../text-vector-support.md) for the
separate implementation and native-capture boundaries.

## Inspection method and limits

The inventory calls `sdocx::parse_detailed`, traverses `stored_pages` and the
decoded document, and reads metadata through `StoredObject::stroke_metadata`,
`ObjectMetadata::flexible_metadata`, `StoredNote::metadata` and
`StoredLayer::metadata`. Across the inspected physical and embedded objects,
common metadata reports no `first_unparsed_field` or residual flexible bytes;
stroke style metadata also reports no unparsed field or trailing bytes. Note
metadata and layer metadata have no residual bytes. This is a statement about
those APIs and fields, not a lossless-decoding claim for every archive record.

A temporary Rust SDK client performed the detailed inventory; it is not a
committed command or required tool. The existing locked-corpus structural check
is reproducible from the repository root when `hf` is available:

```sh
SDOCX_CORPUS_DIR="$PWD/hf" cargo test --offline -p sdocx --features render,serde --test conformance external_corpus_matches_locked_expectations -- --ignored --exact
```

That test verifies the locked hashes and manifest expectations, not every
additional observation above. The temporary inventory generated no SVGs or
render diagnostics, and performed no new visual or native appearance comparison.
