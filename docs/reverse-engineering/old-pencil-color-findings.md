# Legacy Pencil native color controls

Four synthetic Drawing GLV1 host-Mesa captures retain complete 1920 × 1080 RGBA8 frames and native vertex buffers: opaque red, half-alpha red on transparent pixels, half-alpha red over an opaque backdrop, and a pressure ramp over a two-color backdrop. The backdrop controls inject host clears immediately before drawing. Existing Rust tests compare the retained frames across twelve arithmetic profiles.

Two translated pressure-ramp controls retain 42-instance buffers and share the original prior frame. The current Rust regression requires exact frame agreement for both under all twelve profiles. Recorded single-instance replays reproduce their original full frames; these controls do not establish general rasterization or interpolation arithmetic.

[Color capture sources and prerequisites](../../tools/old-pencil-color/README.md) and [translated capture patches and receipts](../../tools/old-pencil-counterexample/README.md) preserve provenance. Native recapture requires local APK libraries, runtime helpers and host Mesa dependencies whose complete closure is not packaged. These are synthetic host controls, not device or saved-note parity evidence. Existing explicit SVG/PDF profiles have separate admission limits; these captures do not establish general export parity.
