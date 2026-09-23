# PDF fonts

Unmodified upstream TTF files, loaded only when exporting PDF. All four styles
(regular, bold, italic, bold italic) are included for each family.

- Roboto: https://github.com/googlefonts/roboto/tree/38062f4b4a0be4346d07a928408da21602545e9e/src/hinted
  Apache 2.0; see `Roboto-LICENSE.txt`.
- Roboto Mono: https://github.com/googlefonts/RobotoMono/tree/895ec691990d041dd727c7b5afa3ce56525d98e6/fonts/ttf
  SIL OFL 1.1; see `RobotoMono-OFL.txt`.

These cover Latin, Greek and Cyrillic, not every writing system. No fonts are
fetched from third-party services at runtime. Native callers can load the same
files with `--font` to supply the same named faces. Generic-family fallback
can still differ because the CLI also discovers system fonts.
