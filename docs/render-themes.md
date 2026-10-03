# Document rendering themes

`RenderTheme::resolve` selects the page background before choosing foreground
and template defaults. Page-level color takes precedence over document-level
color in every mode. Auto preserves the stored paper; without a stored color it
uses light paper.

Explicit modes adapt canonical neutral paper colors when compatibility is not
explicitly disabled. Dark maps white (`#ffffff` or `#fcfcfc`) to the existing
renderer dark paper (`#252525`); Light maps black (`#000000`, `#010101`, or
`#252525`) to `#fcfcfc`. Custom colors and compatibility-disabled paper remain
unchanged. Foreground defaults follow the resulting paper, not the requested
mode. Missing compatibility metadata retains adaptation for older documents.

This is an export policy, not a claim that Samsung uses all these aliases.
Samsung's decompiled `BackgroundColorUtil.isComposerViewDarkTheme` gates paper
adaptation on dark-mode compatibility and an exact canonical white match.
`res/values/colors.xml` defines that white as `#fcfcfc` and composer dark as
`#010101`. The renderer uses `#252525` for export paper; the native page surface
has not been verified separately from composer UI chrome.

## Native evidence

Local APK sources are under `scratch/apk-analysis-decompiled/sources/`:

- `com/samsung/android/support/senl/nt/composer/main/base/util/BackgroundColorUtil.java`:
  `isDarkModeNote`, `isComposerViewDarkTheme`, and `getBackgroundThemeColor`.
- `com/samsung/android/sdk/pen/util/color/SpenReverseColorTheme.java`:
  `getColorByLightControl` reverses HSL lightness outside the inclusive 0.4–0.6
  interval and preserves alpha.
- `scratch/native-dark-theme-color.txt`: `libSPenBase.so` native
  `DarkColorTheme::GetColor` and `getColorByLightControl` corroborate that
  algorithm. The native constants at offsets `0x41734` and `0x41724` are
  exactly the f32 representations of 0.4 and 0.6.

## Foreground adaptation

Handwriting, text, foreground spans, links, shape fills and outlines share the
same native lightness-reversal primitive. It preserves hue/chroma and the
caller's alpha.

The renderer applies that candidate only when compatibility permits adaptation
and it improves contrast against the resolved surface. Existing bright ink on
dark paper is therefore preserved. Explicit Light mode can adapt white content
when moving a canonical dark page onto light paper. Auto and Light on an
already-light page preserve stored foreground colors. This contrast guard is
an explicit export policy; native per-object selection of color themes is not
yet proven to use this guard. The conversion itself matches 785 independently
executed native cases, including threshold boundaries and alpha values.

Text in a highlighted box, filled shape, table cell, or code block receives its
local surface. Table fills use native heading overrides, owned colors or table
defaults and preserve alpha. Text contrast uses the fill composited over paper;
a transparent fill retains the actual paper color, including custom paper.
See [cell background selection](reverse-engineering/table-code-findings.md#cell-background-selection).
Compatibility-disabled documents preserve explicit foreground colors; missing
colors still receive readable defaults.

Run the optional APK oracle from the repository root:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/theme_native.py
```

The oracle reuses the existing hash-pinned ARM64 loader. Native `GetColor`,
`ColorToHSL`, and `HSLToColor` execute unchanged; only libc remainders are host
supplied. `conformance/theme-colors.json` runs in ordinary Rust CI without APK
binaries. Native LightColorTheme is also checked to preserve its input.

## Highlighters and thumbnails

Top-layer highlighters use one vector Darken batch on light paper and Lighten
on dark paper. The dark-paper rule preserves light ink and keeps colored marks
visible on black. Rust pixel tests cover normal/replay equality, and PDF tests
verify a Lighten blend state without image objects.

Static [page-capture findings](reverse-engineering/capture-composition-findings.md#top-pass-composition)
trace Lighten selection when the converted background is dark and the page has
no PDF background; otherwise capture selects Darken. The native shader maps
those modes to component-wise maximum and minimum. This establishes mode
selection and blend math, without captured appearance, sampling or edge-coverage
parity. The SDK shares its background-based blend policy across SVG, replay and
vector PDF. Samsung's separate
[Standard list-page PDF path](reverse-engineering/standard-pdf-composition-findings.md#highlighter-blend-reaches-the-pdf-image-object)
always selects Darken for its highlighter batch, including on dark paper.

Library import and regeneration both render the first page in Auto mode,
independent of the viewer selection. Catalog version 2 invalidates old thumbnail
references; existing recovery removes the orphaned derived assets, preserving
original notes, collections and memberships. Thumbnails regenerate when notes
are opened. Stale results from a replaced document are rejected.

## Validation and parity limits

`web/tests/e2e/themes.spec.ts` renders every visible page of the four compatibility
fixtures in Auto, Light, and Dark: ten pages and thirty mode/page combinations.
It compares normal and replay pixels in Chromium, unchanged path geometry and
embedded image references, selected PDF paper colors, and vector blend states.
Image-free source pages remain image-free PDFs; source images remain embedded.
The formatting fixture includes tables and code blocks on later pages.

Rust regressions separately cover contradictory stored backgrounds, compatibility
opt-out, explicit black and white handwriting/text, alpha-preserving shape paint,
white highlights on dark pages, dark highlights on light pages, custom inherited
table paper, highlighter visibility, and vector PDF/replay preservation.
Thumbnail tests cover canonical mode, stale results, and catalog migration without
losing originals or collection membership.

Native evidence limits:

- Exact page-surface dark color: the APK composer resource is `#010101`, while the
  existing export default is `#252525`. Composer chrome does not establish page
  surface output, which remains unverified.
- Per-object selection: the native conversion primitive is verified, but the
  renderer's contrast guard and canonical paper aliases are explicit export
  policies rather than verified Samsung selection rules.
- Dark-paper highlighter blending: native capture mode selection and shader math
  are statically traced, but resulting Samsung pixels remain unverified. The
  SDK's dark-paper vector PDF uses Lighten, differing from Standard list-page
  PDF's unconditional Darken.
- Hyperlink palette: native `DarkColorTheme::GetColor(0xff0054ff)` returns the same
  blue (included in the checked fixture). That color is therefore preserved and
  can remain low contrast on dark paper. A separate native link/palette rule has
  not been established; it is not claimed to be fixed by lightness reversal.

The interface theme remains independent of document mode. Auto follows stored
paper, not the browser color scheme. Firefox fails the fountain-mask visibility
and maximum-blending checks; see [fountain vector parity](reverse-engineering/fountain-parity.md).
