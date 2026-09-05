# Common object properties and metadata

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37, `arm64-v8a/libSPenModel.so`.
This investigation uses native reader, writer and getter code. No new SDOCX
or Samsung PDF fixtures were available for this change.

Every mapped object chain begins with a type-0 frame. The bounded decoder
exposes that frame through `StoredObject::base_metadata`, as well as the `base`
member of explicit object metadata such as `FormulaMetadata`.

## Serialized properties

`ObjectBaseBinaryHandler::m_ApplyOwnBinary_Property` at `0x2db510` reads a
length-prefixed property mask. The helper at `0x2db650` copies at most two mask
bytes into a zero-initialized value, while advancing over all declared bytes.
The stores at `0x2db59c`–`0x2db5e8` assign the following base-data members.
The getters independently identify their meanings.

| Mask bit | Base-data offset | Getter | SDK field |
| --- | --- | --- | --- |
| 0 | 62 | `IsRotatable`, `0x2cbe14` | `rotatable` |
| 1 | 64 | `IsSelectable`, `0x2cc03c` | `selectable` |
| 2 | 65 | `IsMovable`, `0x2cc150` | `movable` |
| 3 | 61 | `IsVisible`, `0x2cb1e8` | `visible` |
| 4 | 60 | `IsReplayable`, `0x2cabd8` | `replayable` |
| 6 | 67 | `GetTemplateProperty`, `0x2ccc48` | `template` |
| 7 | 66 | `IsFlipEnabled`, `0x2cc258` | `flip_enabled` |
| 9 | 193 | `GetLockState`, `0x2d29f4` | `locked` |
| 12, inverted | 208 | `IsRemovable`, `0x2ca0c8` | `removable` |

For bit 12, the reader tests `0x1000` at `0x2db598`, produces a boolean with
`cset eq` at `0x2db5dc`, and stores it at `0x2db5e8`. A clear bit means
removable. All other named properties in the table use a set bit for true.
`GetOwnBinary` at `0x2daad8` writes the corresponding property bits.

An empty or short mask zero-extends: absent visibility is false and absent
bit 12 makes removable true. Some native getters return different fallback
values when the object implementation pointer is missing. Those branches do
not define the meaning of a successfully decoded serialized object.

Object visibility differs from layer visibility. Objects use positive bit 3;
layers use inverted bit 0. Formula drawn bounds explicitly skip invisible
source and answer strokes; see [formula rendering findings](formula-rendering-findings.md).

Bits 5 and 8 store booleans at base-data offsets 63 and 192. Earlier notes
called these clippable and ATT. This pass did not confirm those names through
getters, so they remain available only in `property_mask`. In particular,
`HasSavedAttValue` at `0x2d22f0` reads implementation offset 135, a different
location from base-data offset 192. It does not establish bit 8's meaning.
Unknown bits, including mask bytes beyond the native reader's two-byte copy,
remain intact in the SDK.

## Fixed fields and extensions

The fixed layout follows the variable property and field masks:

| Field | Encoding |
| --- | --- |
| Format version | `u32` |
| UUID | `u16` UTF-8 byte count and bytes |
| Modification timestamp | `i64` |
| Bounds | four `f64` values, left/top/right/bottom |
| Replay timestamp | `i32` |
| Resize mode | `u8` |

`ApplyOwnBinary` at `0x2db0e0` stores the replay timestamp into base-data offset
72 at `0x2db3ec`, and the resize byte into offset 56 at `0x2db40c`.
`GetReplayTimeStamp` at `0x2cc36c` and `GetResizeOption` at `0x2cb348` read those
members. The resize getter accepts 0–2 and returns 0 for larger values; the
binary reader stores the byte directly. The SDK retains `resize_mode_raw`
without applying that getter normalization. Enum names and timestamp units
remain unresolved.

Flexible bit 0 is a four-byte rotation. Its reader branch at `0x2db744`–
`0x2db75c` reads into base-data offset 68. Absent rotation stays `None` in the
SDK, preserving the distinction from explicit zero. Bounds and rotation must
be finite.

The SDK retains both complete masks, all remaining fixed bytes, and all
flexible bytes after rotation. These tails end at the declared frame boundary;
they exclude subsequent typed frames and the outer integrity trailer. Later
flexible fields remain undecoded in this change.

## Implementation and verification

`ObjectMetadata` exposes the nine confirmed properties, replay timestamp,
resize byte, masks and bounded extensions. Existing page rendering does not
yet apply these visibility flags automatically. This metadata is a prerequisite
for the visible-stroke bounds needed by formula rendering.

Synthetic regressions cover independent property bits across five mask bytes,
inverted removable behavior, zero-extension, raw resize values, UTF-8 identity,
extension preservation, and fixed/rotation truncation that cannot borrow bytes
from flexible data or later frames. They also retain non-finite rotation
rejection. Real-file visual conformance remains a separate task.
