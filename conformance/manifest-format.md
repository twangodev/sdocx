# Corpus manifest

`corpus.json` has a `version` of `1` and a `fixtures` array. Each fixture has a
unique `id`, two assets named `sdocx` and `reference_pdf`, and an `expected`
object. Each asset has a corpus-relative `path` and lowercase `sha256` digest.
Fixture IDs contain only ASCII letters, digits, underscores and hyphens.

The structural checker validates the schema in ordinary CI without downloading
fixtures. The external test verifies both hashes, parses the note, checks the
reference PDF page count, and evaluates the expectations. The visual runner
reads the same manifest for file identities and visible page counts.

## Expectations

`stored_pages` and `visible_pages` are required positive integers. The visible
count must not exceed the stored count. The remaining checks are optional:

| Field | Check |
| --- | --- |
| `title` | Exact structured-note title. An empty string asserts an empty title. |
| `body.minimum_characters` | Lower bound on Unicode character count in the structured-note body; defaults to zero. |
| `body.required_text` | Every listed substring must occur in the structured-note body. |
| `flow.text_sections` | Exact number of stored text sections. |
| `flow.hyperlinks` | Exact count of document-level hyperlink spans. |
| `flow.required_link_targets` | Every listed target must occur in a hyperlink's custom data. |
| `flow.tables`, `flow.code_blocks` | Exact counts of decoded objects directly in document-level text flow. |
| `flow.required_table_text` | Every listed substring must occur in a table cell. |
| `flow.required_code_text` | Every listed substring must occur in a code-block body. |
| `page_objects.strokes` | Total decoded strokes across stored document pages. |
| `page_objects.images` | Total legacy and native image placements, including unresolved placements. |
| `page_objects.text_boxes` | Total standalone text-box elements. |
| `page_objects.shapes`, `page_objects.lines` | Total decoded native shapes and lines. |
| `diagnostics` | Exact counts by diagnostic code name; defaults to no diagnostics. |

Omitting `body` or `flow` skips that group's checks. Including either group
requires the corresponding structured data to exist. Required-substring lists
default to empty; individual required strings cannot be empty or whitespace.
Omitting a count skips that check, while zero asserts absence. Unknown fields
are rejected by the structural checker, so misspelled expectations do not
silently disappear.

Page-object counts use the parsed stored pages before flowing text is composed
onto visible pages. They exclude embedded table/code objects and text inside a
shape. These counts check parser preservation; the visual comparison checks
appearance. Neither alone establishes complete fidelity.

For example, the `expected` section of a four-page drawing fixture could be:

```json
{
  "stored_pages": 4,
  "visible_pages": 4,
  "page_objects": {
    "strokes": 3,
    "images": 0,
    "text_boxes": 2,
    "shapes": 12,
    "lines": 8
  },
  "diagnostics": {
    "UnsupportedShapeFeature": 2
  }
}
```

Those numbers are illustrative. Establish expectations by checking the saved
note, parsed structure, diagnostics and matching Samsung PDF. Do not register
the example as a real fixture or automatically accept current parser output
as correct.

Diagnostic counts are positive integers. A new diagnostic, a changed count,
or disappearance of an expected warning fails the structural check. When a
feature is implemented, review the output and update its expected diagnostic
count in the same change. Code counts do not distinguish messages or objects
within one diagnostic category.

## Adding a pair

1. Put the Samsung-exported `.sdocx` and PDF in the `hf/` dataset submodule.
2. Add the asset paths and hashes under a new fixture ID in `corpus.json`.
3. Record page counts and the specific object/text checks exercised by the note.
4. Run the structural and visual checks from [the corpus instructions](README.md).
5. Push the dataset commit to Hugging Face, then commit its `hf` submodule
   pointer and the manifest changes together in the SDK repository.

The former 17-column `corpus.tsv` has been replaced. Custom visual-runner
manifests passed with `--manifest` must use this JSON format.
