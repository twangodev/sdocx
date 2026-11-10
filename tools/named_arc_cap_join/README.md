# Saved Arc outline caps and joins

The effective-target trace and audit are identical to the dashed Arc control and
are shared from `../named_arc_dash`; the cap/join source manifest and receipts
remain specific to this probe.

The probe creates a moved type-20 Arc with red fill, width 6 and translucent `0x80009955` outline. It sets and saves cap and join controls, reloads the frame through native Model, and records Drawing's SkPaint setter calls and path submissions. Controls cover each nondefault cap and join independently and two combined cap/join cases with dash 2. Native Drawing submits a center-closed fill and separate open outline, and invokes SkPaint's cap and join setters with the selected values. The dashed cases also retain the observed dash effect. Five allocation fills (0, 85, 165, 255, 0) reproduce every receipt and frame.

Run `python3 tools/named_arc_cap_join/replay.py /absolute/path/to/arm64-v8a /absolute/path/to/unicorn/lib`. The source manifest covers the rustc dependency set and pins Unicorn; native libraries and host libm are hash-checked by the capture. Reached PLT bindings are audited, but indirect targets and compiler version are not pinned. The canvas is a recording substitute: the probe does not execute device rasterization, whole-document loading, or native PDF export. SVG/PDF tests check transport and active graphics state, not pixel parity. Other widths, colors, and combinations extrapolate from these and earlier outline/dash controls. Compound outlines, arrowheads, unsupported caps and joins, and nonmatching geometry retain fallback.
