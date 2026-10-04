# Pencil3 V1 saved-stroke source authority

## Evidence and scope

Samsung Notes 4.4.45.37 APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
These are independently reviewed static ARM64 traces, without native execution,
geometry captures or appearance comparison. Pencil3's extracted library matches
its APK entry. Addresses are ELF virtual addresses.

| Source | SHA-256 |
| --- | --- |
| `libSPenPencil3.so` | `41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenPenCommon.so` | `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d` |
| Decompiled `pen/pen/SpenPen.java` | `75fe3f0d71e4ea5e979bd5a6b6803454283571e01c6ffcfecbb91a45e5adb9c7` |

The [registry](pen-selection-findings.md#built-in-native-registry) resolves
Pencil3 independently of Pencil/Pencil2. This finding covers one selected V1
profile and the single ordinary saved-stroke route, rather than every version,
queued batch, preview or live gesture. The [recorder](stroke-recording-findings.md)
establishes saved source channels separately from these rendering inputs.

## Saved channels feed generated particles

Pencil3 `createPenInst`, `0x106678`, constructs the plugin. Its main slot 232
relocation `0x11a8f0` binds `GetStrokeDrawableGL`, `0x106404`. The getter reads
the configured version and `versionTable` (`0x106418`, GOT `0x11c298 →
0x120b18`); entry 1 selects V1 construction at `0x10649c`.

Drawing `ObjectDrawing::redrawIPen`, `0x82abc`, requires count 1 and
StrokeType != 1 for its ordinary single-object branch. It obtains that drawable
through pen slot 232 and calls secondary-interface slot 40 (`0x82b54–0x82b70`).
V1 installs that interface at `this+8`; relocation `0x11abc8` binds its saved
adapter thunk `0x1088d8`, which adjusts `this` and calls `0x1087d8`.
The adapter reads stored tool, count, XY, pressure, time, tilt and orientation
(`0x108808–0x108854`), then constructs a MotionEvent with those arrays and
the first timestamp (`0x108860–0x108884`). Primary slot 144, relocation
`0x11ab78`, dispatches to V1 saved-event redraw `0x107b04`. The adapter and
thunk ignore the inner result and return true; that is not rendering success.
Saved redraw calls libc `srand(1)` at entry (`0x107b3c`, `0x107b50`), before
its event/canvas guards. Its reached `drawPoint` and `drawLine` calls
(`0x107e54`, `0x107f58`, `0x108724`) reach `drawPoint`, `0x108338`, which
calls `rand` three times (`0x108398`, `0x1083d0`, `0x1083ec`). Two values
produce size-dependent offsets around sampled XY. The third becomes a signed
remainder by 16. RTV1::AddPoint receives the resulting tuple (`0x108454`).

That consumer appends X, Y, half-size, random remainder and alpha
(`0x10c880`, `0x10ca30/40`, `0x10caf4`). Its descriptor and update bind a
four-component point attribute plus alpha, with count=bufferFloats/5
(`0x10c378/390`, `0x10cca8–0x10ccd8`). The bound vertex source at `0xf9730`
uses the fourth component to select a row of the 16-piece brush pattern.
This value is neither a saved random seed nor another source coordinate.
Fixed reseeding does not establish whole-application repeatability: the C
random implementation, call ordering, profile and sampling remain separate
conditions. No Android-versus-host sequence or numerical parity is established.

## Paper coverage has separate runtime ownership

Drawing `SetDepthmap`, `0x80564`, stores a supplied bitmap at renderer+48.
Saved drawing passes it at `0x81e00–0x81e0c` to `SetPenDepthmapBitmap`,
`0x750c8`. For StrokeType 0, pen slot 232, GL slot 56 and depth-map interface slot 16
reach V1's setter
(`0x1089c8`, interface relocation `0x11ac10 → 0x1089ac → 0x108954`).
The renderer's upstream document/paper producer is not resolved here.
Separately, SDK `SpenPen.setDepthMapBitmap` (`:1043–:1056`) passes its supplied
Java Bitmap to PenCommon JNI `0x41120`. It creates a graphics bitmap from
those pixels and dimensions (`0x41188`, `0x41224`), calls the same interface
(`0x41254`), then releases its local reference (`0x41260`). This proves a
configuration input, without establishing a concrete app paper-file producer.

V1's GL setter releases its old member 488 bitmap and references an accepted
new pointer (`0x108964–0x108998`); its destructor releases it (`0x106f68`).
Saved redraw supplies it to PenReturnCallback (`0x107c4c`), whose destructor
copies callback+56 into queued message+16 (PenCommon `0x50524–0x50538`).
The message's actual execute slot (`0x774d8 → 0x4676c`) supplies it through
the RT depth-map interface before Draw (`0x46920–0x46970`). RTV1 caches it
at +248; null or same-pointer setting is a no-op (`0x10c7e4–0x10c854`).
These bodies do not establish queue-wide failure or lifetime guarantees.
RTV1 Draw selects that supplied paper bitmap or default member 240
(`0x10cde4–0x10ce74`). The default 150×150 paper comes from embedded
`g_backgroundPtn` (`0x11c2c0 → 0xf3c8c`, copy `0x10c248–0x10c29c`);
the brush pattern separately comes from `g_penPtn` (`0x11c2b8 → 0x12c8c`).
The bound fragment source, `0xf9948`, multiplies paper-red coverage, brush-red
coverage, input color alpha and point alpha, with factor 1.5. This identifies
coverage inputs rather than proving texture or compositing equivalence; see
the separate [opacity findings](pen-opacity-findings.md).

The paper bitmap is distinct from an [editable Painting source](painting-source-findings.md)
and does not replace saved XY. No inspected route proves this runtime input
is serialized in an ordinary stroke. Preserve original samples, optional
channel presence, tool, pen/settings identities and source bytes independently
of generated particles. Rust currently uses the [pressure approximation](stroke-rendering-findings.md#current-sdk-behavior)
for Pencil profiles. [Native PDF stroke bitmaps](native-pdf-stroke-findings.md)
are derived output; original vector channels and configured coverage inputs
remain separate source boundaries.
