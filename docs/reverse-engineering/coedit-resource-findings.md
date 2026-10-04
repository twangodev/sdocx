# Coedit resource references and received content

## Evidence and scope

These are static ARM64 and decompiled Java findings from Samsung Notes 4.4.45.37,
APK SHA-256 `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Addresses are ELF virtual addresses in the named library. The APK member
`lib/arm64-v8a/libSamsungNotesCoedit.so` is 2,474,392 bytes and was compared byte-for-byte
with that pinned APK member; the other inputs use the established pinned ARM64 files.
No native execution, network download, archive roundtrip or appearance comparison is claimed.

| Library | SHA-256 |
| --- | --- |
| `libSamsungNotesCoedit.so` | `ca947736284c75035a4a89e4d654a604b42e2ff6947b0b13317824bbadaa9e60` |
| `libSPenWordDocCoedit.so` | `82a73d24732efe4f5c3c385b9fcb0ccfb05970507ba50039d252c2abb7f0c2af` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenXmlSerializer.so` | `7be7af380ae378f91c0e565dcc022479c3f87aa965a5190f245f4d006cb6f36a` |

## XML creates a current local resource namespace

SamsungNotesCoedit `CoeditNoteWrapper::SetXml`, `0x11b6f8`, calls its CoeditNote member's
slot 24 at `0x11b77c`; WordDocCoedit relocation `0x69598` resolves it to `SetXml`, `0x4167c`.
That note constructs WNoteXmlSerializer with format 1 (`0x40ef0–0x40efc`), then SetXml
invokes its slot 24 at `0x4173c`. XML relocations `0x17b8e0` and `0x17b8c8` close
WNoteXmlSerializer::SetXml `0x15c540` → root Parse `0x159a28`. Construct assigns the nested
media serializer this same WNote's manager, from note impl +224, and format 1
(`0x1598dc–0x1598f4`); CONTENT_FILE_LIST dispatch calls its Parse at `0x159b48`.
This is a positive reader route, not proof of ordering relative to downloads or every session.

XML MediaFileManagerXmlSerializer::Parse `0x1151a0` clears ContentFileList (`0x115204`),
not the whole media map in this body. Its lookup at `0x1152e0` uses ContentFileData +64,
REAL_FILE_HASH (`0x113c28–0x113c2c`, GOT `0x17fbf8`). HASH at +48 and FILE_HASH at +80
are separate strings (`0x113c08–0x113c14`, `0x113c40–0x113c44`). An absent map key and
format 1 obtain a new local ID, set metadata count 100, rewrite the managed name using
that numeric prefix and retained suffix after `@`, then insert metadata
(`0x115400–0x1154fc`). Existing-key handling is separate; absence is not a disk check.
Fresh Model metadata has payload flag +58 false (`0x28b9ec`); checked XML attributes do
not change it. Nonzero-format MediaFileXml parseElement bypasses its inline-byte route
(`0x113ce4–0x113cec`). Pending namespace metadata does not reconstruct original source bytes.

## Image hash lookup and live installation are conditional

XML CoeditObjectShapeXmlSerializer::ParseFillEffectAttribute decodes a Base64 effect and
calls ApplyBinaryByCoedit (`0x13967c`). Model FillImageEffect `0x3b9614` enables its hash
mode; the common reader's manager slot 136 lookup (`0x3b9354`, relocation `0x491ac8`)
is GetFileIdByHash `0x295938`: current metadata ID or -1, without allocation, binding,
file validation or count increment. Ordinary XML IMAGE_ID parsing is a separate route.
The temporary effect's ImageCommon starts without a manager (`0x3b8160`, `0x2b506c`);
AddImage(int) can store that ID with a null path (`0x3b93ec`, `0x2b5580`, `0x2b567c`).

Live UpdateFillEffectByXml (`0x13969c` → Model `0x3ac440`) calls FillImageEffect::UpdateByXml
(`0x3ac604` → `0x3b9828`), clears the previous ID and stores CopyImageInfoForCoedit's
returned internal index (`0x3b9868–0x3b9880`). ImageData::CopyForCoedit `0x2b8c44` copies
the media ID first; with a nonnull manager it requires GetFilePathById success, but ignores
Bind(int) and RefreshSize results (`0x2b8c64–0x2b8ca8`). Without a manager it copies source
dimensions. Both successful branches copy saved nine-patch rectangle, reference width and
flags. GetFilePathById constructs the managed path without checking file existence
(`0x28fa20–0x28fa44`), so a live reference does not require successful source-size refresh.
The inner update bool is checked (`0x3ac608`), but outer XML ParseFill ignores it, checks only
the changed flag for cache clearing, then returns true (`0x13969c–0x1396e4`). Parser success
therefore does not certify live installation. These references differ from the
[saved coedit geometry baseline](image-effects-findings.md#original-placement-precision-and-coedit-span-state).

## Download completion attaches to managed local bytes

Java `GrpcController.java:51–52` → `CoeditGrpcPresenter.java:124–125` →
`ntnl/coedit/CoeditManager.java:142–143` forwards download requests to JNI. SamsungNotesCoedit
JNI calls its manager at `0x1b4ccc`. Its completion consumer onContentFileDownloadSuccess
`0x149fbc` reverses the two string arguments at `0x14a0b0–0x14a0b8`; the worker's first
string is the file operand, while its second feeds the later result callback. Manager forwarding
(`0x16d504`) reaches the worker (`0x1837e4`), whose slot 56 call (`0x183960`, relocation
`0x24af80`) resolves to CoeditNoteWrapper::AttachContentFile `0x11b878`. Its admitted call
(`0x11b8f4`) reaches WordDocCoedit CoeditNote `0x45348` → CoeditContentFile `0x4b388` →
WDoc WNote::AttachContentFile `0x94e10` → Model MediaFileManagerNew::AttachFile `0x28f1e8`.

AttachFile requires the supplied file to exist, hashes it and queries the current map
(`0x28f2bc–0x28f374`). Existing-hash metadata chooses its current managed destination.
If that destination exists, it skips rename and destination rehash, sets payload +58 true
and returns the existing ID (`0x28f418`, `0x28f6a8`). Otherwise Rename must return integer
zero (`0x28f424–0x28f428`). Fresh keys allocate a local ID (`0x28f540`), conditionally rename
with the same zero-success rule, then create count 1/payload true metadata (`0x28f720`).
A nonnegative result does not certify equality of an existing destination and supplied bytes.
Only that ID condition triggers listener broadcast (`0x4b3dc`); worker success separately
requests deletion of its first path (`0x1839c0`), without proven aliasing or executed deletion.

## Pending callbacks and ordinary saving use different gates

WordDocCoedit Image registers after OnInsertedByXml (`0x39c9c`) or its partial content-change
gate (`0x39a64–0x39a74`). isAllContentFileExist `0x39ca0` checks current main/border/original
IDs (`0x39cf4`, `0x39d34`, `0x39d74`), skipping only -1. Manager slot 184, Model relocation
`0x491af8`, resolves to CheckBoundFileExist `0x28b504`: current path lookup then File::IsExist.
Failed checks populate a pending ID set; false query registers this +56 and stores the
supplied content owner at +80 (`0x39b34–0x39b40`), ignoring registration's returned result.
OnContentFileAttached `0x39b50` removes a matching ID; empty set plus nonnull owner invokes
RefreshImageSizeForCoedit, RequestRedraw, deregistration and owner clearing (`0x39ba8–0x39c20`).
It neither rechecks disk/bytes nor verifies broadcast-owner and pending-query manager identity.
Web independently checks thumbnail and HTML IDs (`0x3c1cc`, `0x3c208`); Link checks thumbnail
(`0x3ac00`), through the same availability slot. Their [FileAttachers](card-source-findings.md#bound-resources-have-consumers-beyond-card-drawing)
are separate from the image branch's ImageCommon/ImageData resources.

Ordinary selected-fill writing reaches GetBinary (`0x3a9070`, relocation `0x494af8`), resolves
its internal index to current media ID (`0x3b90f0–0x3b90fc`) and writes that ID in ordinary
mode (`0x3b91c4`); coedit mode instead emits a hash. The callback does not rewrite these IDs.
Border/original writers use GetMediaId (`0x3a4630`, `0x3a4794`), subject to their path/index
admission gates; [card writers](card-source-findings.md) likewise record current resource IDs.
WDoc SaveCache's media call (`0xadd64–0xadd7c`) reaches Model saveItem `0x28fe5c`.
Count 100 passes its existing live-count gate (`0x28feb0–0x28fec8`), subject to the current
file-accessibility check (`0x28fee0`) before the manifest record. The existing
[media binding/save gates](vector-retention-findings.md#native-opaque-records-wrappers-and-resources) remain separate
from ZIP payload addition, which needs NoteZip and payload +58 true (`0x29003c–0x290080`).
Count 100 alone cannot establish byte inclusion.
The [ordinary save/publication boundaries](vector-retention-findings.md#native-page-files-during-save)
still apply; ComposerModel.java:804–824 performs its save before separate saveCoeditCache.
Rust types archive media IDs/names/recorded hashes and selected image IDs, but has no coedit
arrival codec or listener. [Caller-owned original page bytes](vector-retention-findings.md#original-page-bytes-are-external-to-the-parsed-model)
remain independent of current native IDs, managed bytes, pending metadata and callbacks.
