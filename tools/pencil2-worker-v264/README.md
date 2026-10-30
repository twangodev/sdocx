# Native factory layout and RGBA control

Follow the v219 preparation and run without Python optimization:

```sh
python3 tools/pencil2-worker-v264/probe.py
PENCIL_LAYOUT=1 python3 tools/pencil2-worker-v264/probe.py
PENCIL_LAYOUT=6 PENCIL_TARGET=2 python3 tools/pencil2-worker-v264/probe.py
python3 tools/pencil2-worker-v264/draw_rgba.py
PENCIL_SKIP_DRAW=1 python3 tools/pencil2-worker-v264/draw_rgba.py
python3 tools/pencil2-worker-v264/copy_rgba.py
PENCIL_SKIP_DRAW=1 python3 tools/pencil2-worker-v264/copy_rgba.py
python3 tools/pencil2-worker-v264/copy_rgba_rgba.py
PENCIL_SKIP_DRAW=1 python3 tools/pencil2-worker-v264/copy_rgba_rgba.py
```

Graphics `CreateBitmap` at RVA `0xc04ac` forwards its character pointer into the bitmap constructor and to a virtual method; the constructor symbol names two separate `TextureMemoryLayout` and `TextureTargetLayout` arguments. The factory reads these from the caller stack, not registers x8 and x9. The inherited `p.call` writes all arguments into numbered registers, so its ninth and tenth array elements do not supply those stack arguments. The v255 source factory explicitly wrote zeros to the stack; v261's `RGBA8888` string was not a format selection. The later destination captures inherited stack values until explicitly set here. This does not identify values supplied by a real page.

The first three controls explicitly write stack layouts (0,0), (1,0), and (6,2), respectively. Their guest `glTexImage2D` requests are R8/RED/UNSIGNED_BYTE, R16UI/RED/UNSIGNED_SHORT, and RGBA8/RGBA/UNSIGNED_BYTE. The host callback forwards these null-data calls unchanged and checks `glGetError` immediately afterward: on this Mesa host the results are 0, `GL_INVALID_OPERATION`, and 0. The error read consumes the GL error, and could include an earlier unconsumed error; it is an intervention and does not establish portable support or a usable texture for the failing combination. The factory returns a non-null object in each case. These observations do not determine enum names or their full range.

The drawing control configures (6,2) in the native source factory and leaves the GL allocation unmodified. It asserts the guest RGBA8 request, Run/Update/Draw entry order, and exact equality of its painted frame with the retained v256 RGBA8 intervention fixture. Its Draw-skipping hook records entry and immediately returns before the Draw body; the frame is transparent. The copy controls invoke native `CopySubRegionFromBitmapRT` with a factory RGBA8 destination: one copies the inherited R8 source, the other a factory RGBA8 source matching the v256 frame; each destination readback equals its respective source byte-for-byte for drawn and skipped controls. They check framebuffer completeness and final GL error, but cannot rule out errors consumed earlier. The copy scripts deliberately terminate at the capture boundary, bypassing inherited post-capture assertions, and do not validate queue or object destruction.

All observations use emulated ARM64, a synthetic queue and manually substituted canvas, host callbacks, unpinned Mesa, and the v255/v261 runtime limitations. Null-data initial values are host observations. These controls do not establish actual saved-page format selection, page lifecycle, real production export, or device parity. The v259 matched-input Rust and export comparison remains a separate diagnostic; this native allocation result makes its factory-format intervention unnecessary for this one synthetic setup, not a production parity proof.
