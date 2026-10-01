# Samsung reference fixture capture

Create each note in Samsung Notes and export the same saved note as both
`.sdocx` and PDF. Keep these files together and use a new fixture ID when
changing their contents. Record the Samsung Notes version, device/OS, page
size, font names and export settings in the external dataset's notes.

Use native objects from the app's tools and record their settings. Preserve
original image assets alongside the exports. If a setting is unavailable in
the installed app version, record that limitation rather than substituting a
screenshot or edited SDK output. Existing fixture IDs and captured content are
listed in [the corpus guide](README.md) and locked in [the manifest](corpus.json).

## Validate and register

1. Reopen the saved source in Samsung Notes and inspect every PDF page for the
   intended content. Confirm that the PDF came from the same saved revision.
2. Store the pair in the `hf/` dataset submodule, following the corpus license
   and attribution conventions. Keep APKs, fonts and document binaries out of
   the reverse-engineering knowledge base.
3. Compute SHA-256 for both files. Inspect parsed structure and visible page
   counts before adding the matching entry to `corpus.json`; do not guess counts
   from ZIP entry order. Text expectations are optional. Add exact page-object
   counts and any confirmed diagnostic counts using the
   [manifest format](manifest-format.md).
4. Run structural conformance and the visual runner with the new ID. Preserve
   diagnostics, reference/SDK images, tool versions and font hashes. Add focused
   synthetic regressions for each confirmed parser or renderer defect.
5. Commit and push the dataset changes to Hugging Face, then commit the `hf`
   submodule pointer and manifest changes together in the SDK repository.

Successful parsing and similar pixel scores establish coverage of these cases;
broader compatibility still requires additional app versions and documents.

## Pen identity and coverage

Record the pen identity and settings shown by the debugger, including pressure,
tilt, opacity and fixed-width options used in the note. Preserve drawing order
for overlapping or destination-dependent effects. The native registry and alias
mapping are documented in
[`pen-selection-findings.md`](../docs/reverse-engineering/pen-selection-findings.md).

The `02-shapes-and-dot-calibration` fixture contains FountainPen calibration
marks. Those observations do not establish parity for other pens or settings.
Registry recognition, implementation support and reference-validated appearance
are distinct checks; page-wide pixel scores do not establish full pen coverage.
