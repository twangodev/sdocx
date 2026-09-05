# Android motion-event conversion

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37's
`com.samsung.android.sdk.pen.view.SpenMotionEvent`, ARM64 `libSPenBase.so`
and `libSPenDrawing.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation). The Java constructor
was freshly decoded in fallback mode because ordinary decompilation had
omitted its body. Labels below refer to that constructor's DEX offsets.

This adapter copies pressure and the two pen axes without normalization,
promotes Java float coordinates to native doubles, and carries two time
channels. Native millisecond getters subtract down time; nanosecond getters
do not. This precedes the separate
[view coordinate transforms](view-input-transform-findings.md) and
[pen-action filters](stroke-input-findings.md). It does not establish the
physical calibration of device measurements or every later event mutation.

## Java collects current samples and pointer-major history

The constructor's `L69` block copies `getActionMasked()`, `getToolType(0)`,
`getDownTime()` and `getEventTime()`. The `La8` loop allocates one `EventInfo`
per pointer and copies these channels:

| EventInfo field | Android getter |
| --- | --- |
| X, Y | `getX(pointer)`, `getY(pointer)` |
| Raw X, raw Y | `getRawX(pointer)`, `getRawY(pointer)` |
| Pressure | `getPressure(pointer)` |
| Tilt | `getAxisValue(25, pointer)` |
| Orientation | `getAxisValue(8, pointer)` |
| Pointer ID | `getPointerId(pointer)` |
| Minor, major | `getToolMinor()`, `getToolMajor()` |

The current minor/major calls have no pointer argument, and tool type is
read once from pointer 0. These details limit assumptions about mixed-tool
multitouch events. Pressure, tilt and orientation are float assignments;
there is no clamp, degree conversion or pressure curve in this constructor.

`L128` allocates `historySize * pointerCount` historical entries. For flat
index `k`, `L138` calculates:

```text
pointer_index = k / historySize
history_index = k % historySize
```

Each pointer's entire history therefore precedes the next pointer's
history. For two pointers and three history entries, the order is
`(0,0), (0,1), (0,2), (1,0), (1,1), (1,2)`.

The historical getters receive both indices for X/Y, pressure, axes 25/8
and minor/major. Historical time receives only the history index. The loop
does not assign historical raw X/Y or pointer IDs. `L1da` copies source,
button state and flags into the wrapper.

## Nanosecond retrieval has a millisecond fallback

The constructor sets `msToNano` to 1000000. On the first check it separately
looks up `semGetEventTimeNano()` and
`semGetHistoricalEventTimeNano(int)` on Android's MotionEvent class.

Current nanosecond time initially equals `eventTime * 1000000`. A successful
reflected call replaces that value before `L9b`; an unavailable method or
invocation exception leaves the product in place. Each historical entry
similarly starts with `historicalEventTime * 1000000` and optionally
replaces it before `L175`.

Thus the presence of a native nanosecond field does not prove that the
input had submillisecond precision. The two reflection checks are separate;
this trace does not establish which methods a particular device exposes.

## Native conversion preserves sample channels

Base `SPen::ConvertMotionEvent`, `0xe1e90`, looks up the wrapper and nested
`EventInfo` fields by their JNI names and descriptors. It allocates current
coordinate records with a 72-byte stride at `0xe23dc`–`0xe23f4`.
The recovered in-memory layout is:

| Offset | Representation | Field |
| --- | --- | --- |
| 0 | 64-bit integer | Event time |
| 8 | 64-bit integer | Nanosecond event time |
| 16 | Float | Orientation |
| 20 | Float | Pressure |
| 24 | Float | Tilt |
| 28, 32 | Floats | Minor, major |
| 36 | 32-bit integer | Resampled state |
| 40, 48 | Doubles | X, Y |
| 56, 64 | Doubles | Raw X, raw Y |

This is a runtime structure, not the SDOCX stroke record layout.

For each current pointer, the converter stores the wrapper's two scalar
times at `0xe24d8`. It does not obtain current times from the nested
`EventInfo`. It promotes X/Y at `0xe24e0` and `0xe2500`, and raw X/Y at
`0xe2520` and `0xe2540`, from floats to doubles. The promotions preserve
the Java float values; they do not restore additional coordinate precision.

Pressure, tilt and orientation are stored directly at `0xe2570`,
`0xe258c` and `0xe25a8`. The converter calls the pointer-array MotionEvent
constructor, `0xbfa28`, at `0xe26ac`. That constructor copies each complete
72-byte coordinate record through `memcpy` at `0xbfbfc`; it does not
rescale those channels. Action and tool-type handling has separate branches
at `0xbfa80`–`0xbfac4` and `0xbfb00`–`0xbfb08`, so the sample-copy finding
does not imply that all event metadata is unchanged.

The historical conversion loop follows the flat Java array order at
`0xe2714`–`0xe2894`. It reads each nested entry's two time fields, promotes
X/Y at `0xe2858`–`0xe285c`, and passes the supplied float channels to
`MotionEvent::AddBatch` at `0xe2884`. No pressure or axis arithmetic occurs
in this loop.

`AddBatch`, Base `0xc01b4`, writes the two time arguments at `0xc022c`,
pressure/tilt at `0xc0248`, and orientation at `0xc024c`. It writes the
supplied X/Y pair into raw X/Y at `0xc0218` and copies it into X/Y at
`0xc0234`. Historical raw coordinates consequently equal historical X/Y
at this boundary; they are not independent Android raw measurements.
The later view transforms change X/Y without updating that raw pair.

Base `GetHistorySize`, `0xc07a8`, divides the flat historical-record count
by pointer count at `0xc07bc`. The two-index historical getters use
`pointer_index * historySize + history_index`, as shown by the
minor/major getter at `0xc075c`–`0xc0768`. This agrees with the Java order.
Current samples remain in a separate vector; the adapter does not append
them to history.

## Resampled metadata has two gates

Java initializes each `isResampled` field to false. It attempts reflected
`PointerCoords.isResampled()` access when its static check finds
`Build.VERSION.SDK_INT >= 34`; lookup or invocation failure leaves false.

Native conversion applies a further check at `0xe2608` and `0xe282c`.
When the unsigned value at Base `0xf6c34` is at least 35, it stores the
Java boolean as 0 or 1; otherwise it stores -1. `System::SetSDKVersion`,
`0xc757c`, and `GetSDKVersion`, `0xc7588`, identify this global as the
native configured SDK version. Its initialization and the downstream
interpretation of the three states remain separate research targets.

## Millisecond getters subtract down time

The pointer-array constructor stores the supplied down time in MotionEvent
implementation member 16 at `0xbfad8`. Its getters have different rules:

| Getter | Entry | Returned value for a valid sample |
| --- | --- | --- |
| `GetEventTime()` | `0xc0634` | First current sample's time minus down time |
| `GetHistoricalEventTime(index)` | `0xc0670` | Selected historical sample's time minus down time |
| `GetEventTimeNano()` | `0xc0650` | First current sample's nanosecond time |
| `GetHistoricalEventTimeNano(index)` | `0xc06a8` | Selected historical sample's nanosecond time |

The millisecond subtraction instructions are `0xc0648` and `0xc0698`.
The nanosecond getters load record offset 8 at `0xc065c` and `0xc06c8`
without subtraction. Invalid historical indices return zero. With down
time 1000 and event time 1032, the millisecond getter returns 32; the
fallback nanosecond getter returns 1032000000. These are arithmetic
consequences of the adapter, not captured device measurements.

The [stroke recorder](stroke-recording-findings.md) calls the millisecond
getters at Drawing `0xb7e68` and `0xb7ee8`. It passes their low 32 bits
to `ObjectStroke::AddPoint` at `0xb7ea0` and `0xb7f20`. It does not select
the nanosecond channel or subtract the first recorded point's timestamp
in these append loops. The separate
[insertion-time millisecond flag](stroke-insertion-findings.md) changes
metadata without rescaling the array.

## SDK implications and validation

For stored-stroke replay, retain the decoded time channel and its mode.
This live-input adapter is not evidence for multiplying a saved timestamp
by 1000000, subtracting the first point again, or normalizing pressure and
axes during decoding. Device input, event transformation, model recording
and binary channel encoding are separate boundaries.

The APK digest and both native library byte streams were verified.
Fresh constructor output, JNI field names, record stores, getter arithmetic
and recorder imports were checked against the APK and ARM64 instructions.
Disposable reconstruction checked pointer-major ordering and the timestamp
example. No native execution or new SDOCX fixture was used, and no SDK code
changed. Remaining input targets include resampled-state consumers,
special action/tool remapping and nanosecond consumers.
