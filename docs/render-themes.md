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
  algorithm; the two floating-point constants still need direct verification.

The shared paper resolver fixes contradictory page/document precedence and
foreground defaults. Native foreground conversion, local text backgrounds,
highlighter compositing, canonical thumbnail generation, and end-to-end export
validation remain part of the active theme work. The old near-black foreground
heuristic is not native color parity.
