# Common object properties and metadata

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37, `arm64-v8a/libSPenModel.so`.
This investigation uses native reader, writer and getter code, without a paired
SDOCX/Samsung PDF comparison.

The modification-time save trace also uses `libSPenWDoc.so` and `libSPenBase.so`.
Pinned SHA-256 values are:

| Library | SHA-256 |
| --- | --- |
| Model | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| WDoc | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| Base | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |

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
| 5 | 63 | `IsOutOfCanvasEnabled`, `0x2cbf28` | `out_of_canvas_enabled` |
| 6 | 67 | `GetTemplateProperty`, `0x2ccc48` | `template` |
| 7 | 66 | `IsFlipEnabled`, `0x2cc258` | `flip_enabled` |
| 8 | 192 | `IsFloatDrawnRect`, `0x2d2190` | `float_drawn_rect` |
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
The ordinary drawing dispatcher also skips hidden objects before traversing
container children. Page decoding follows that confirmed gate while
preserving their physical records; see [object drawing findings](object-drawing-findings.md).

Bit 5's getter identifies permission for placement outside the canvas. Java's
`SpenObjectBase.getOutOfViewEnabled` calls `ObjectBase_isClippable`; its native
bridge at `0x307a98` directly calls `IsOutOfCanvasEnabled`. Bit 8's getter
identifies floating drawn bounds. These names describe the native API;
automatic clipping and float-layout behavior are not established by these getters.
In particular,
`HasSavedAttValue` at `0x2d22f0` reads implementation offset 135, a different
location from base-data offset 192. It is a separate state.
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
without applying that getter normalization. Timestamp units remain unresolved.
The native collection comparator uses a separate signed 64-bit replay order
at base-data offset 80, rather than this timestamp; see
[object drawing findings](object-drawing-findings.md).

Java `SpenObjectBase` declares `RESIZE_OPTION_FREE = 0`,
`RESIZE_OPTION_KEEP_RATIO = 1`, and `RESIZE_OPTION_DISABLE = 2`.
`ObjectBase_getResizeOption` at `0x307808` dispatches through vtable slot 72;
the `ObjectBase` vtable relocation at `0x492150` points to `GetResizeOption`.
The SDK exposes these names through `ObjectMetadata::resize_mode`, with
`ObjectResizeMode::Other` retaining unknown values rather than normalizing
them to free resizing.

Flexible bit 0 is a four-byte rotation. Its reader branch at `0x2db744`–
`0x2db75c` reads into base-data offset 68. Absent rotation stays `None` in the
SDK, preserving the distinction from explicit zero. Bounds and rotation must
be finite.

The SDK retains both complete masks, all remaining fixed bytes, and all
flexible bytes after rotation. These tails end at the declared frame boundary;
they exclude subsequent typed frames and the outer integrity trailer. Later
flexible fields are available through a separate `flexible_metadata` call.
Their native map, bounded bundle decoding and remaining unknowns are recorded
in [optional object findings](object-flexible-findings.md).

## Modification time during native saving

Static ARM64 calls and relocations establish a selected ordinary-object save
route, without executing a native save/reload. WDoc `WPageSaveHandler::Save`
calls `Save_LayerDoc` at `0xd6044`, which calls Model `WLayer::Save` at
`0xd6c2c`. The latter calls `Save_Objects_WDoc` at `0x341de8`; its non-container
branch supplies compatibility false and document type 2 to `WriteDefaultObject`
(`0x355558–0x355560`). That modern branch invokes virtual `ReadyForSave` before
binary size and bytes (`0x35505c–0x3550ac`). ObjectBase and ObjectStroke slot-424
relocations resolve to `ObjectBase::ReadyForSave`, `0x2d1500`; other types can
override preparation, as ObjectShape does at `0x399f14`.

On this DocumentType-2/non-compatible branch, queried size (`0x355080`) selects the
payload boundary; a negative writer return fails, while its nonnegative count
does not replace that size (`0x3550b0`). With coherent queried/current size,
the 32-byte trailer starts there and requested length is size +32
(`0x355164–0x3551ec`). Those `File::Write` results are unchecked here, so this
framing contract does not establish completed I/O or a successful save.

The base preparation requires an implementation and attached context. It skips
when the sync requester returns any nonzero integer or the coedit requester
returns true (`0x2d1514–0x2d1550`). Otherwise virtual `IsChanged` true or an
existing zero modification value triggers `GetTimeStamp` and replacement of
base-data offset 152 (`0x2d1554–0x2d157c`). A clean nonzero value is preserved
by this body. Missing requester functions default to zero/false here; missing
context skips replacement. Model `GetSyncMode`, `0x2ac27c`, and `IsCoeditMode`,
`0x2ac2dc`, identify those requesters. The WDoc note constructor installs them
at `0xa07d4/0xa0904`; callback bodies `0xa75d4/0xa77f8` read captured note
implementation offset 976's sync integer and offset 854's coedit byte.

For the base/stroke implementation, `IsChanged`, `0x2cf2c0`, reads implementation
byte 240. `HasUnsavedChanges`, `0x2cf320`, reads distinct byte 241.
`ObjectBaseImpl::SetChanged`, `0x2d74f4`, and `ClearChanged`, `0x2d7f78`, notify
available context callbacks and set/clear these flags, without a direct clock
call or modification store in either body. This does not establish all callback
side effects or the complete save-success clear lifecycle.

These replacement values are realtime microseconds: Base `GetTimeStamp`,
`0x9a1f0`, uses clock ID 0 and seconds × 1,000,000 + nanoseconds / 1,000. The
[recording findings](stroke-recording-findings.md#voice-synchronization-uses-append-time-and-original-objects)
pin the clock instructions. This producer contract does not normalize every
historical or manually assigned value: `SetModifiedTime`, `0x2cc524`, directly
stores its `i64` argument at `0x2cc574`. The separate replay `i32` above remains
a different timestamp domain.

ObjectStroke's modern writer calls the base writer at `0x2e56cc`, which calls
`ObjectBaseBinaryHandler::GetOwnBinary` at `0x2d12ac`. The latter emits offset
152's eight bytes at `0x2dabc4–0x2dabcc`. `WriteDefaultObject` then derives its
32-byte trailer from UUID text and the current signed decimal modification time
(`0x3550c4–0x355168`); see [integrity findings](integrity-findings.md). A changed
time can change both fixed bytes and identity trailer while geometry stays the
same. Equal clock results remain possible. This is an identity digest, rather
than proof of all object-content integrity or byte-preserving archive output.

`RequestUpdateHash`, `0x2d42a4`, is a separate explicit operation requiring a
present coedit requester returning true. It timestamps offset 152 and stores
`ExtractHash`'s result at offset 216 (`0x2d4318–0x2d4368`), without a dirty or
sync requester test in its body. The inspected save chain does not establish a
call to it. The compatible writer separately stamps dirty-or-zero modification
with an attached context and zero/absent sync requester (`0x2da094–0x2da0d0`);
its dirty argument comes from virtual `IsChanged` (`0x2d06c8–0x2d06f4`), and
that bounded branch has no coedit skip. Rust retains `modified_time_raw` and
verifies identities against saved values; parsing does not perform this native
save preparation or substitute a current timestamp.

## SDK behavior and validation

`ObjectMetadata` exposes the eleven confirmed properties, replay timestamp,
resize byte, masks and bounded extensions. Page decoding excludes hidden
recognized objects and their subtrees from the semantic model while retaining
their physical records. Automatic formula rendering remains unimplemented.

Synthetic regressions cover independent property bits across five mask bytes,
inverted removable behavior, zero-extension, raw resize values, UTF-8 identity,
named resize modes with unknown-value preservation, extension preservation,
and fixed/rotation truncation that cannot borrow bytes
from flexible data or later frames. They also retain non-finite rotation
rejection. These regressions do not establish real-file visual parity.
