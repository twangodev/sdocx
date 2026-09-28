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
`#010101`. The renderer's existing `#252525` export paper is retained pending
verification of the native page surface separately from composer UI chrome.

## Native evidence and remaining work

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
caller's alpha. The former RGB-sum threshold has been removed.

The renderer applies that candidate only when compatibility permits adaptation
and it improves contrast against the resolved surface. Existing bright ink on
dark paper is therefore preserved. Explicit Light mode can adapt white content
when moving a canonical dark page onto light paper. Auto and Light on an
already-light page preserve stored foreground colors. This contrast guard is
an explicit export policy; native per-object selection of color themes is not
yet proven to use this guard. The conversion itself matches 785 independently
executed native cases, including threshold boundaries and alpha values.

Text in a highlighted box, filled shape, table cell, or code block receives its
local surface. Table cells without an owned background inherit the actual paper
color, including custom paper. Compatibility-disabled documents preserve
explicit foreground colors; missing colors still receive readable defaults.

Run the optional APK oracle from the repository root:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/theme_native.py
```

The oracle reuses the existing hash-pinned ARM64 loader. Native `GetColor`,
`ColorToHSL`, and `HSLToColor` execute unchanged; only libc remainders are host
supplied. `conformance/theme-colors.json` runs in ordinary Rust CI without APK
binaries. Native LightColorTheme is also checked to preserve its input.

## Remaining work

Top-layer highlighters use one vector Darken batch on light paper and Lighten
on dark paper. The dark-paper rule preserves light ink and keeps colored marks
visible on black. Rust pixel tests cover normal/replay equality, and PDF tests
verify a Lighten blend state without image objects. Dark-paper Lighten is an
export policy; a Samsung dark-paper highlighter export is still needed to prove
its native compositing mode.

Canonical thumbnail generation and end-to-end export validation remain part of
the active theme work. Native per-object color-theme selection and exact dark
paper color remain explicitly unverified above.
