# Shared text fonts

Unmodified upstream TTF files used by the Rust text pipeline and vector PDF
export. Each family includes regular, bold, italic and bold italic faces.

- Roboto: https://github.com/googlefonts/roboto/tree/38062f4b4a0be4346d07a928408da21602545e9e/src/hinted
  Apache 2.0; see `Roboto-LICENSE.txt`.
- Roboto Mono: https://github.com/googlefonts/RobotoMono/tree/895ec691990d041dd727c7b5afa3ce56525d98e6/fonts/ttf
  SIL OFL 1.1; see `RobotoMono-OFL.txt`.

These cover Latin, Greek and Cyrillic. Other writing systems need additional
fonts; bundled faces do not establish Samsung's device-specific fallback order.
