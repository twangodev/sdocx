# Vector retention boundaries

## Evidence and meaning of retention

These findings trace the current Rust parser, inspection APIs and vector output.
The native sections concern Samsung Notes 4.4.45.37 ARM64, APK
SHA-256 `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Native addresses are ELF virtual addresses, before relocation. Other native
contracts are linked to their separate findings.

| Native library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| `libSPenSDoc.so` | `e1b2aa314e69ea5ba284aafec4326967075c0f2ec878cb2265b96abfd817de10` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenBodytext.so` | `27324ca3807f07e0c1d0647b23eb9af1296762a8c9d892ee486f37b1eb9543f0` |
| `libSPenWidget.so` | `cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9` |

Native opaque-record, hierarchy, custom-object, resource, clipboard and legacy
content-conversion findings are static traces. Only the bounded unknown own-frame writer/reader was executed; no
complete native archive round trip or Samsung appearance comparison is claimed.

| Representation | What it retains | What it does not establish |
| --- | --- | --- |
| Original archive or uncompressed entry bytes | Opaque records, encoded geometry and resource bytes | Semantic decoding or a renderer |
| `StoredPage` and `StoredObject` | Object types, byte ranges, integrity trailers and ordered children | Ownership of the bytes identified by those ranges |
| High-level `Document` | Supported selected objects, typed fields and some bounded opaque fragments | Every saved object, field, layer or resource |
| Generated SVG/PDF | Supported drawable geometry and text transport | Original binary identity or complete native appearance |

Retaining vectors includes their resource and placement identity. Original source
images remain images; retaining those bytes does not introduce rasterization.
Reconstructed vector pen geometry can preserve a drawable ink representation
without being a copy of an original saved outline.

[Coedit resource findings](coedit-resource-findings.md) distinguish XML-created
local IDs, live resource references, received managed bytes and ordinary-save
payload admission. Attachment callbacks do not certify original archive identity.

## Original page bytes are external to the parsed model

[`StoredObject`](../../crates/sdocx/src/storage.rs) stores `payload_offset` and
`payload_size`. Its `payload(page_bytes)` method borrows the corresponding range
from a caller-supplied uncompressed `.page` entry. `ParsedDocument` owns the
physical indices, high-level document, typed note, manifests for page order and
diagnostics, but no original page-byte store.

The [archive parser](../../crates/sdocx/src/container.rs) reads each page into a
local buffer, decodes it, and returns only semantic values and physical indices.
The buffer does not survive in the returned `ParsedDocument`. Ordinary `parse`
also discards the physical indices and parse report. Consequently, an opaque
page-object payload remains recoverable only when the caller retains or reopens
the matching original input. Serializing offsets does not serialize their bytes.
The physical page-header projection also keeps only the low 32 bits of each
variable-length mask, alongside its byte count; it does not own the original
high mask bytes or page flexible data.

The browser has a separate ownership boundary: its
[`DocumentSession`](../../crates/sdocx-wasm/src/lib.rs) retains a
[debugger source](../../crates/sdocx-wasm/src/debugger.rs) containing original
archive bytes. This permits later structural inspection in that session. It
does not add source ownership to the standalone Rust `ParsedDocument` API.

### Native page files during save

Ordinary native saving builds from working cache. SDK `SpenWNote.java:2412–2416`
calls the file-save JNI route, which resolves the note and calls
`WNote::Save(path,true,false)` (`0xe6120`–`0xe6128`). The bool overload forwards
`skipCorruptedFileInfo` (`0xe6660`–`0xe6678`); the directory route supplies
file-output false (`0xe6288`–`0xe6290`). An actual app caller,
`SpenWordDocument.java:193–199`, uses saveAsDirectory. These calls do not
establish every application save policy.

WNote checks its state/cache-directory preparation, then SaveHandler
(`0x95408`–`0x95444`). The handler constructs NoteZip from working cache,
checks SaveCache, then promotes temporary files before ZIP or directory copy
(`0xad5a4`–`0xad608`, `0xad660`, `0xad72c`). SaveCache checks media saving,
page saving and fresh note serialization in that order
(`0xadd64`–`0xadde0`). Member payload forwarding therefore coexists with new
note/manifest/trailer state; this route does not forward the original ZIP.

The page decision uses current UUID `<page-id>.page` under the working root.
`WPageManager::savePage`, `0xb2ca0`, calls live WPage::Save only if IsChanged,
corrupted-info, or absence of that cache file is true (`0xb2e44`–`0xb2e64`).
Loaded state and HasSnapSavedData are not part of that rewrite predicate.
An existing unchanged, noncorrupted page bypasses live writing. Main NoteZip
selects every admitted page; an optional second ZIP selects only snap-present
or PageFileUpdated pages (`0xb2e7c`–`0xb2ea0`, `0xb2f10`). These local Add
returns are ignored, so member selection alone does not prove payload I/O.

Pending snapshot promotion changes which cache bytes are packaged.
RenameTempFiles processes updated, restore-snap or has-snap pages
(`0xb35dc`–`0xb3600`). Updated pages use the current temporary extension;
otherwise the source is `<page-id>.page.ssf` (`0xb3648`–`0xb36a4`). Ordinary
save uses a null target extension, promoting that physical file to `.page`
before packaging. An unchanged page can thus bypass live writing while its
pending snapshot replaces the selected `.page`. SnapSave instead targets
`.ssf` (`0xaeda0`–`0xaedd4`); its SaveCache snapshot bool selects media
SnapSave (`0xadce0`–`0xadcf4`), rather than changing the page rewrite predicate.

Live writing requires page implementation, its context at impl+304 and a media
manager. If unloaded, Save checks LoadObject(false,false) before output
(`0xd566c`–`0xd56f4`). LoadObject's first bool true alone selects an accessible
`.page.ssf`; false uses ordinary `.page` (`0xd52dc`–`0xd5340`). The writer opens
`<impl-cache>/<UUID>.page<tempExt>` in `wb+`, then reconstructs current common,
flexible/custom and layer records and a page hash (`0xd5704`–`0xd5794`,
`0xd5e40`, `0xd6044`, `0xd61c4`). It has no whole-original-page tail-copy path.
The existing [opaque-record contracts](#native-opaque-records-wrappers-and-resources)
still bound what those lower writers retain.

Failed live Save stops page admission, with no local older-page fallback
(`0xb2e68`, `0xb2f30`–`0xb2f34`). Custom, layer, hash and WithInfo returns are
checked, while some individual Write/Seek/Close returns are ignored.
RenameTempFile constructs source/destination filenames and calls raw rename,
without serializer or copy fallback (`0xd77e4`, Base `0x96ca0`). Its source
must be accessible; an existing destination is unlinked before the checked
rename (`0xd794c`–`0xd7950`, `0xd79b8`–`0xd7a1c`). Failure therefore does not
establish transactional rollback. ZIP/file transport is checked later and
carries the selected current file bytes, conditional on successful I/O;
outer compression, password and EndTag framing remain separate.

Cache initially can come from a supplied input stream through checked NoteUnzip
extraction, or from a saved directory (`0xa2410`–`0xa245c`). It can later hold
promoted snapshots or reconstructed pages. Changed-page unload itself checks
save/promotion before clearing live layers (`0xd7250`–`0xd72cc`). Bypassing a
later serializer therefore does not certify original archive-entry identity.
These source-only traces establish no executed native round trip or universal
recovery. Caller-owned original archive/page bytes and the browser source
carrier remain distinct from native current-cache or editable serialization;
the parsed Rust model does not acquire their complete resource namespace.

### Native directory and file publication

Ordinary app saving and file sharing use distinct publication routes.
`WordDocRepository.java:439–454` calls `DocumentFileManager.saveWordDocument`;
its lambda (`:123`–`:125`) reaches `SpenWordDocument.save` (`:193`–`:199`), which
uses `saveAsDirectory`. Repository `SaveResult.isFileSaved` is set after normal
return, while caught exceptions populate the result. A `.sdocx` pathname alone
therefore does not establish ZIP output.

The [native page-save route](#native-page-files-during-save) constructs selected
current cache members first. WDoc SaveHandler, `0xad4e4`, then stages output at
its selected target plus a generated temporary extension (`0xad610`–`0xad630`).
Its file branch checks ZIP and appended fresh EndTag byte count
(`0xad644`–`0xad684`); its directory branch checks selected-file-list copying
(`0xad710`–`0xad730`). Both enter the same target exchange.

SaveHandler builds `target + "_back"` (`0xad76c`–`0xad784`). When the target is
accessible, it attempts removal of a previous backup, then requires
`Rename(target, backup)` to return zero (`0xad958`–`0xadaa4`). Backup cleanup
failure logs and does not itself stop that rename. It next checks
`Rename(stagedOutput, target)` (`0xad798`–`0xad7a4`). An inaccessible target
skips the old-target backup stage. Failed publication attempts backup-to-target
restoration if the backup is accessible, then still returns failure
(`0xad810`–`0xad878`). Successful publication instead attempts backup cleanup;
cleanup failure logs but does not veto OnSaved (`0xada2c`–`0xadb50`). This is the
producer corresponding to the [open/recovery backup route](#native-note-opening-and-recovery-sources),
not a complete rollback or atomic multi-rename guarantee.

Base File::Rename and File::Unlink return their raw libc integers unchanged
(`0x96ca0`–`0x96cb8`, `0x96b4c`–`0x96b5c`), so zero means success. After handler
success, WNote calls `SyncFileSystem` (`0x9546c`), whose Model helper `0x2861dc`
tail-calls `sync`. Later state-reset failure can still make WNote::Save fail
(`0x95484`–`0x95488`). These observations do not establish checked per-file
durability or exact original-source publication.

A concrete file-sharing caller, `ShareUriHelper.java:96–112`, generates a unique
share path and calls two-argument `DocumentCopyUtils.copy` for `.sdocx` source.
Its default is coedit false (`:58`–`:60`); the four-argument helper (`:161`–`:175`)
uses native MakeFile only for an actual Sdocx directory with false. Other input
uses BaseUtils.copyFile; coedit-true directory input instead opens a live note
and calls saveAsFile. Its outer true follows normal return from a void helper
that catches inner exceptions, rather than a checked save-result bool.
`DocumentFileManager.copy` (`:174`–`:175`, lambda `:98`–`:100`) reaches SDK
`SpenWNote.makeFile` (`:380`–`:391`), whose JNI calls WDoc MakeFile
(`0xe63d8`–`0xe63e4`). These bounded routes do not describe every shared output.

WDoc `WNote::MakeFile`, `0x95af0`, branches on actual source-directory status
(`0x95cf4`–`0x95cfc`). For a directory, it roots NoteZip at that supplied source
and checks page then media GetZipList (`0x95d10`–`0x95d30`), before adding
`note.note` and `end_tag.bin` (`0x95d34`–`0x95d88`). Its Construct and these Add
results are not locally tested. The page selector reads saved `pageIdInfo.dat`
and emits each nonempty ID plus `.page` in manifest order
(`0xb7430`–`0xb7574`); it does not rewrite live pages, promote `.ssf`, or recursively
scan directory files. Model's media selector, `0x2945c0`, applies saved manifest inclusion gates to
`media/<filename>` (`0x29487c`–`0x2948dc`). Selected saved membership is distinct
from [live media admission](#native-opaque-records-wrappers-and-resources)
and arbitrary original archive entries.

MakeFile stages at `target + ".tmp"`, checks output construction and ZIP, then
calls `AppendEndTagToFile(source, stream)` (`0x95d94`–`0x95df4`). That helper
reads the entire saved `source/end_tag.bin` into a buffer and submits those
bytes (`0xbb9c8`–`0xbbbe4`), rather than reconstructing a tag from a newly opened
note. It rejects only a zero write result; the concrete Base stream writer can
return a short count or -1 (`0xd0628`–`0xd072c`), so the helper does not certify
full-length completion. Stream Close calls fsync then close
(Base `0xd0610`–`0xd0624`), but MakeFile ignores its result at `0x95dfc`.
No executed short-write or replacement loss is established.

Before publishing, MakeFile attempts removal of an accessible existing target
(`0x95e00`–`0x95f28`). A false directory-removal result logs and still continues;
its regular-file branch logs on raw Unlink zero and skips logging on nonzero,
then both proceed. Final `Rename(temp, target)` is checked for zero
(`0x95f2c`–`0x95f38`), with no local backup restoration. Nondirectory native input
instead uses checked SPenCopyFile (`0x95e4c`–`0x95e58`); its Model String overload
copies the same chunk buffer through read/write (`0x2852b8`–`0x285340`), with a
same-path success shortcut. It does not decode native records or locally
validate ZIP format.

Saved-directory packing can transport selected vector, opaque and original
asset payloads without redraw while creating new outer ZIP state. Live save
can reconstruct native records first. Original supplied archives, current saved
directory members and published output remain separate source units; neither
helper success nor output existence establishes retention of every original
archive member or unselected orphan file.

### Native note opening and recovery sources

The SDK exposes `discardSnapSavedData`, `skipCorruptedFileInfo` and coedit mode
(`SpenWNote.java:951`, `2843–2862`; JNI `0xe4108`–`0xe414c`). An actual normal
repository route allows snapshots: `WordDocRepository.java:111–124` opens when
the input path exists or hasSnapSavedData is true, supplying discard=false;
`SpenWordDocument.java:269–278` also supplies skip=false. A maintenance caller,
`DeleteCoeditNoteUseCase.java:142–147`, supplies discard=true. These are distinct
observed policies, not every app open path or a successful-recovery certificate.

Path ConstructLoad checks InitCachePath, ConfigureCache and ConstructImpl in
that order (`0x90688`–`0x906b0`, `0x90774`–`0x90778`). The original path at
impl+16 and working cache at impl+48 are separate carriers. ConfigureCache
first checks the original. For a directory, CheckEndTag checks quick-save
recovery on that original directory before ordinary EndTag reading
(`0xbc65c`–`0xbc6ac`); a file instead uses its stream. Invalid originals enter
restoreBackupFile, which checks the original path plus `_back`, its EndTag,
and the restoration helper (`0xa1c60`–`0xa1c70`, `0xa1f3c`–`0xa1fcc`). After
failed restoration, impl+976==1 rejects configuration; otherwise the cache-only
helper rejects discard=true or absent working cache (`0xa1c74`–`0xa1d3c`,
`0xa20a8`–`0xa20b8`). Its remaining branch can return true even without an
admitted snapshot; that return alone does not certify a complete note.

With an original source, CheckOverwriteFlag forces overwrite for absent cache
or skip=true. Otherwise it reads the cache's ordinary EndTag and compares core
note modified time with the supplied EndTag (`0xa26d4`–`0xa2768`). The +32
field is identified by FillEndTagData copying impl+208, the named GetModifiedTime
reading that member, and Model's wire reader immediately after note ID
(`0xa3c38`–`0xa3c40`, `0x96c58`–`0x96c64`, `0x2a7ef0`–`0x2a7f18`). It is
separate from display timestamps. Equality is a single-field check, not an
archive-content hash or byte-identity certificate; read failure or inequality
sets overwrite=true.

Only equality reaches CheckSnapSavedData on that route. A snapshot attempt
requires cache state exactly 1, discard=false and accessible
`<cache>/end_tag.bin.ssf` (`0xa2554`–`0xa25a8`). A skipped attempt sets
overwrite=false and does not delete snapshot files. An admitted attempt clears
the supplied EndTag before checking ReadFile(cache,true,true); success sets
impl+793 and overwrite=false, failure sets overwrite=true
(`0xa2600`–`0xa263c`). There is no local restoration of the cleared EndTag.
Subsequent cache extraction/merge checks receive the overwrite flag
(`0xa21fc`–`0xa2240`, `0xa2410`–`0xa245c`); this does not establish an admitted
note with lost metadata or universal fallback. The stream ConstructLoad route
instead creates a UUID cache and supplies overwrite=true, without CheckSnap in
its ConfigureCache body (`0x90c18`–`0x90c58`, `0xa22f8`–`0xa2320`).

ConstructImpl applies the selected EndTag, initializes components, then passes
impl+793 and skip to Load (`0xa38f4`–`0xa392c`). Load checks media loading before
note loading and then pages (`0xa85b8`–`0xa85f4`, `0xa8824`). The media virtual
slot resolves to Model Load(bool,bool), which directly selects its manager-cache
`mediaInfo.dat[.ssf]` (`0x292bdc`–`0x292c44`). The same first bool selects
working-cache `note.note[.ssf]`; note data, title and body textboxes come from
that selected file (`0xa8900`–`0xa892c`, `0xa9190`–`0xa9218`). loadPageIdInfo
passes impl+793 to select `pageIdInfo.dat[.ssf]` (`0xaa0e0`–`0xaa0ec`,
`0xb2378`–`0xb2398`). These selected-file routes have no local ordinary-file
retry after failure. The flag also reaches PageLoadInfo+24 (`0xa880c`), but
individual pages retain the accessible `.page.ssf` gate [above](#native-page-files-during-save).
Selected metadata suffixes therefore do not imply every page/resource payload
is a snapshot copy or prove their complete namespace. The independent
[version-admission contracts](note-header-findings.md#native-reader-version-authority)
remain separate from this source choice.

Quick-save recovery is a physical-file operation before this snapshot decision.
RecoverQuickSavedData reads `qsave_state.dat`; state0 returns true. After checked
backup listing for nonzero state, state1 checks rollback, state2 checks backup
deletion, and other nonzero states reach cleanup
(`0xbc9ac`–`0xbca58`). Backup-list matching uses ReverseFind('.bak')==length-4
(`0xbe750`–`0xbe75c`), separately from the original-path `_back` sibling.
Rollback strips that suffix, checks target unlink and raw rename; it stops on
failure without undoing earlier member changes (`0xbe8f8`–`0xbe934`). State
reading returns its initialized/read buffer even after logging read errors
(`0xbe62c`–`0xbe634`), and recovery ignores state-file removal and filesystem-sync
results (`0xbca50`–`0xbca58`, `0xbcab8`). Success is not an atomic rollback or
whole-note completeness guarantee. InitializeCacheDirectory does not call this
recovery helper.

The Rust [archive parser](../../crates/sdocx/src/container.rs) selects appended
or archive EndTag, archive `note.note` and `pageIdInfo.dat` from the supplied ZIP.
It does not select external lifecycle/quick-save state, `_back`, `.bak` or these
`.ssf` alternatives. Caller-owned archive/page bytes and the browser source
carrier retain that input, separately from the native chosen recovery/cache
source. A preservation comparison must associate the selected note and page
manifest with its resource namespace and payloads. These static traces establish
no executed recovered-note round trip, corpus mismatch or source loss.

### Native opening normalizes the requested fixed axis

The main composer passes `UUIDUtils.isCoeditUuid(str)` to getOpenParam
(`DocumentServiceManager.java:234–242`). BaseSubManager returns 2160 for that
true input, otherwise mScreenWidth initialized from SpenDocumentDisplayUtils
(`183`, `235–240`). Its getScreenWidth returns the screen's short side: minimum
main-screen Rect width/height, or minimum real DisplayMetrics widthPixels/
heightPixels (`115–131`). This is not composer viewport width or a physical/DPI
unit promise; null context or some unavailable-display branches can yield zero.
Activity-null getOpenParam returns builder defaults before this policy
(`ServiceContractImpl.java:477–506`).

The existing-file/snapshot repository branch passes configured page width and
integer open mode to opening (`WordDocRepository.java:111–124`). Builder
orientation, derived height and PageMode are separate absent-file creation
arguments; that creation branch selects height only for exact LANDSCAPE, not
LANDSCAPE_NEW. The existing route reaches SDK constructLoad through
DocumentFileManager `207–211`, file/a.java `30–35` and SpenWordDocument `269–278`.
SDK `SpenWNote.java:951` names its integer pageFixedAxisSize. Native ConstructLoad
requires a signed request >31 and stores it at noteImpl+128
(`0x904e4–0x904e8`, `0x90674`). These traced callers do not establish every open mode.

Saved note orientation governs the configured load chain. ApplyEndTagData
stores EndTag GetOrientation at noteImpl+188 (`0xa1d84`); the constructor
registers its captured-note requester (`0xa0770–0xa0788`). Relocation `0x1035b0`
points to callable `0xa7548`, which reads that member; Model GetNoteOrientation
uses it (`0x2ab8cc`). The builder's requested orientation is not substituted here.
Note-level scale uses EndTag reference width as i32, but reference height as f32
(`0xa1da8–0xa1dcc`, `0xa1d8c`). FillEndTagData copies width and converts signed
height to f32 (`0xa3c6c–0xa3c80`); page-header width/height instead enter Model as
saved signed integers (`0xd2c90/0xd2cbc`). These are distinct precision contracts.

After fixed/flexible header data and ReadHash, LoadHeader_Scale stores the
requested axis at pageImpl+180 and divides requested/saved dimensions in f32
(`0xd2914`, `0xd379c`, `0xd37c4/0xd3808`). Orientation exactly 1 fixes requested
height and truncates scaled width with FCVTZS (`0xd37f0`); other values fix
requested width and normally truncate scaled height (`0xd384c`). One explicit
exception is request **2256**, which uses FCVTAS nearest ties-away for height
(`0xd3844`); this is separate from the upstream **2160** policy. Exact f32 scale
1 skips transformations but still reaches Model OnLoadScale (`0xd3a10`).
Nonunit scaling also reaches PDF record rectangles (`0xd38f0`), custom SetRect
(`0xd397c`) and type-1 StickyNote collapse rectangles (`0xd39b8`). Existing
[leaf coordinate findings](object-transform-findings.md#double-wire-coordinates-do-not-imply-double-editing-geometry)
cover object loading; this top-level policy does not establish another geometry algorithm.

Scaling has real local dirty producers. Unequal CustomObject SetRect calls its
implementation, writes changed bytes +152/+153 and sends callback 3
(`0x85f50–0x85f60`, `0x8958c–0x89590`). WPage IsChanged reaches their getter
(`0xc75a8`, `0xd1428`, `0x868a4`). Unequal StickyNote collapse updates also call
SetCustomData and callback 4 (`0x8bb90/0x8bbc4`). Nevertheless, successful public
WPage LoadHeader calls ClearPageChangedFlagAll **after** scaling (`0xc7c3c`):
Model/page flags and custom changed bytes are cleared (`0xcffc4–0xcffd8`,
`0xd0068`). Loaded becomes false, HeaderLoaded true, and layer changed flags
are cleared (`0xc7c48/0xc7c54/0xc7c5c`). This does not reverse normalized geometry.
It is a successful enclosing-route fact, not a guarantee for callbacks, failed
header loading or direct scale-helper calls.

Actual WPageManager loadPage immediately follows successful LoadHeader with
LoadObject (`0xb1e48/0xb1e5c/0xb1e70`); not every manager-loaded page remains lazy.
LoadObject snapshots current IsChanged before its loaded shortcut and forwards
that value to LoadLayer (`0xd5158–0xd5168`, `0xd54d4/0xd54e0`). Successful layer
loading with false preservation clears page/custom and layer/object-manager
flags (`0xd0824–0xd0830`, `0x346c20/0x346c48/0x346c58`); child loading receives
current orientation and requested axis (`0xd0874–0xd0894`). Separately, successful
ConstructImpl sets note dirty flags for recovery flag+793 or clears them and
sets cache state 2 otherwise (`0xa3944`, `0xa39a0/0xa39a4`). Its producer belongs
to the [recovery-source contract](#native-note-opening-and-recovery-sources),
not the per-page save predicate.

The [page-save contract](#native-page-files-during-save) selects live writing for
changed/corrupt/missing-cache pages. Loaded state alone does not select it:
unchanged current cache can remain selected despite normalized runtime geometry,
and pending snapshots can replace those cache bytes. Current working cache is
not necessarily the original supplied page or ZIP. When live writing is reached,
Save loads objects if needed and emits current GetWidth/GetHeight
(`0xd56dc–0xd56f0`, `0xd5950–0xd5988`); after successful layer writes it resets
magnification to 1 (`0xd6c58`). Existing opaque-object writer boundaries still apply.

Rust Page construction copies stored header width/height directly
([page.rs](../../crates/sdocx/src/page.rs)); it receives no app screen-axis input.
Saved units, caller-owned original bytes and native normalized runtime state are
separate preservation authorities. These static paths establish neither an
executed native save nor a reason to silently normalize Rust vectors to a screen.

### Native protected carriers

The SDK password save route native-saves a fresh temporary plaintext file before
calling WLockUtil.encrypt and renaming it to the destination
(`SpenWNote.java:2345–2393`). Its input is the current native save described
[above](#native-page-files-during-save), rather than the original imported
archive supplied through byte retention. Public direct-file lock instead
replaces the input's document type before encrypting it; a directory first
becomes a packed `.enc` file (`SpenWNoteFile.java:350–398`). Actual app
SdocXDocumentLocker.lockFile only sets owner ID and LOCKED_WDOC
(`model/document/save/lock/locker/SdocXDocumentLocker.java:43–64`); the WDoc
setters parse/replace EndTag (`0xc00f8`, `0xc042c`). These examined methods do
not encrypt the file. A locked type alone does not identify ciphertext.

WLockUtil.encrypt streams the supplied plaintext through EOF before adding
its readable appendix (`worddoc/util/WLockUtil.java:136–233`). The saved
plaintext length narrows from Java long to signed int. Model Append requires
a positive signed size and a wrapped-key pointer (`0x2a9964–0x2a997c`). It
writes the source EndTag's captured 20-byte EOCD prefix, zero two-byte comment
length and freshly serialized tag after the ciphertext (`0x2a9bc4–0x2a9c00`).
The prefix was captured while parsing the source EOCD
(`0x2a79c4–0x2a79d0`); this outer carrier is separate from encrypted ZIP entries.

The ordinary JADX decryptCore stub is a failed decompilation. Existing-JADX
fallback output and direct DEX code units establish its actual instructions.
The APK's `classes8.dex` SHA-256 is
`6b7498f182bb73e136e5ce512c0035d3fccada94aecb52e84a1c4154dfa09e84`.
DEX method 26301, code_item `0x2e9138`, has 612 code units starting at `0x2e9148`:
Cipher.init uses unwrap mode 4, unwrap selects AES key type 3, then init uses
decrypt mode 2 (`0x0067`, `0x006d`, `0x0072`). The saved signed plaintext size N
selects CipherBlockInputStreamUtil's cap `N + (16 - N%16)` before CipherInputStream
(`0x0075`, `0x00a6`, `0x00ab`; helper code_item `0x1b7cc8`, units 8–12).
For positive N and nonoverflowing signed arithmetic, this includes a full padding
block when N is divisible by 16. The cap limits bytes exposed to the cipher;
BufferedInputStream can physically prefetch beyond it. The readable appendix
is outside this intended cipher-facing extent, rather than decrypted to EOF.

The output loop writes decrypted chunks until EOF or clips the final write to
N minus the prior total (`0x00b4–0x00c8`). This caps output; it does not check
that EOF produced exactly N bytes. Close IOExceptions are logged and the final
local check tests destination existence (`0x00ce–0x00ea`). Signed-size overflow,
provider padding behavior and successful complete output remain unexecuted.

The outer decrypt wrapper then constructs an EndTag utility on the destination
for validation, clears encryption on its retained **source** tag object and
replaces the destination tag with that object (`WLockUtil.java:279–304`).
Model regular-file Replace finds EOCD, seeks EOCD+20 and overwrites zero comment
length plus a freshly serialized tag (`0x2a93ec–0x2a9404`, `0x2a9500–0x2a9528`).
The bounded Append/Replace file branches show no explicit truncation or checked
File::Write counts. Java's void append/replace discard their native booleans;
the examined WDoc JNI bridges return them without false-to-exception translation
(`0xe38c0`, `0xe39cc`). Wrapper return does not certify a complete carrier write.

SDK open passes a validated temporary decrypted path to ConstructLoad for the
separate [recovery/cache selection](#native-note-opening-and-recovery-sources)
and deletes it after construction (`SpenWNote.java:1346–1370,1406–1443,2895–2914`).
Direct-file unlock can additionally change the type and replace EndTag again
(`SpenWNoteFile.java:515–542`). Neither sequence proves original plaintext-byte
identity or atomic file replacement.

Independent [EndTag inspection](end-tag-findings.md#sdk-implementation)
retains raw encryption appendix bytes and their typed wire fields. The Rust
[archive parser](../../crates/sdocx/src/container.rs) instead rejects a nonempty,
structurally decodable encryption blob with ProtectedDocument; its conservative
blob policy differs from native IsEncrypted's positive signed size/key-pointer
predicate. It does not return a successfully decrypted ParsedDocument owning the
protected input. Caller-owned ciphertext, the bounded plaintext temporary with
rewritten EndTag, and fresh current native output are separate source units.
These static traces establish no encrypted-fixture round trip, complete provider
or disk-write validation, original-byte recovery, or visual parity.

## Selected objects and opaque parents

The [page decoder](../../crates/sdocx/src/page.rs) builds high-level objects from
the saved current physical layer. Hidden recognized objects and their subtrees
are omitted from that model. Inactive and hidden records still have physical
indices; explicit semantic inspection requires original page bytes. This agrees
with the recovered [physical-layer selection](page-layer-selection-findings.md).

[`PageObjectContent`](../../crates/sdocx/src/page_objects.rs) has stroke,
element and container variants, with no opaque object variant. Recognized
containers retain child boundaries and order, but do not retain their complete
common metadata. Unsupported parents are not represented as high-level nodes;
supported descendants are traversed separately and appended at that location.
This does not preserve the parent's semantic identity or establish its drawing
contract.

Saved type-4 container rotations are already applied to child geometry, as
[traced in native setters and drawing](object-selection-findings.md#saved-container-rotations-are-applied-to-children).
A second parent rotation would apply them again. That evidence does not establish
transforms or composition for opaque painting, brush or stroke-group objects.
The [support matrix](../rendering-support.md#object-types-and-locations) records
which native types currently have a dedicated semantic or rendering route.

## Page custom objects and attached source

Page flexible [field 18](file-format.md#page-custom-object-list) owns a separate
custom-kind namespace and keyed file/string records, rather than layer objects
or rich-text embedded-object spans. One concrete kind is `SpenStickyNote` (1).
The app saves a separate `SpenWNote` as `stickymemo_<timestamp>.sdocx`, attaches
that file, then appends the custom object (`StickyMemoObjectManager.java:159–192`,
`SmDocumentManager.java:101–102`, `190–200`, `350–355`). WDoc's ordinary attachment
key is `co_attach_file` (`CustomObject::AttachFile`, `0x84ed0`–`0x84eec`);
its keyed attacher binds through the media manager, and its own frame saves
the returned numeric file ID. Collapse bounds and color live in keyed strings;
thumbnail attachment uses the separate key `co_thumbnail_path`.
Composer's kind-1 overlay draws icon bitmaps (`0x400c4c`), separately from that
nested document's source. An icon or thumbnail does not retain its contents.

The nested file has a concrete archive-member route. Model's
`MediaFileManagerNew::Bind(String*)`, `0x28c5dc`, hashes and deduplicates files;
new generic content is copied or renamed into its resource directory
(`BindFile`, `0x28d390`–`0x28d43c`). The new-name helper, `0x28cf0c`–`0x28d0d8`,
keeps the basename suffix after the last `@`, or the whole basename if absent,
and prefixes the new decimal ID plus `@`. A fresh sticky filename therefore
becomes `media/<id>@stickymemo_<timestamp>.sdocx`; deduplication can select an
existing ID/name instead; binding uses the saved numeric ID.

Fresh successful generic bindings initialize live count 1 and payload flag
`+58` true (`0x28cac4`–`0x28cad4`, constructor `0x28bb90`–`0x28bba0`). Current
WDoc `SaveCache`, `0xadc88`, calls the working four-argument media save
(`0xadd64`–`0xadd7c`, Model `0x290f04`). For an admitted accessible file,
`saveItem` requires flag `+58`, then adds `media/` plus its existing filename
to `NoteZip` (`0x290020`–`0x290080`). A manifest record alone does not prove
payload inclusion. Base's stream ZIP route opens that file in `rb` mode,
reads up to 16 KiB, and writes the same buffer/count into the member
(`Stream_ZipFile`, `0x9b7bc`, `0x9b820`, `0x9b8d0`). This route packages nested
source bytes without decoding the note or substituting a bitmap; outer ZIP
framing/password handling remains separate from its payload.

Opening a sticky memo loads its attached document with a cache save path
(`StickyMemoManager.java:1350–1352`). Updating saves that nested document,
then rebinds its saved file (`StickyMemoObjectManager.java:342–352`), so its
current source need not equal the first creation's bytes. The archive route
above is conditional on successful binding, admission and I/O; no whole
sticky archive round trip or every deduplicated state was executed. Reference
release and [old-manifest cleanup](#native-opaque-records-wrappers-and-resources)
remain separate from next-save payload admission.

### Loaded custom-file maps

The saved keyed file map is an input to later attachment, rather than a bound
source-file certificate. WDoc `CustomObjectImpl::ApplyOwnBinary`, `0x8a654`,
reads keys and signed IDs, skips saved `-1` (`0x8a7e4`–`0x8a834`), and calls
`FileAttacher::ApplyFileId` for other IDs (`0x8a888`). Model's helper,
`0x2b48e4`, only stores the integer. It neither resolves a path/hash nor binds
or validates it. Skipping saved `-1` creates no fresh key; it does not delete
an existing key when applying a frame to an existing map.

`LoadCustomObject` appends an accepted binary, calls OnAttach with its page
context, binds the instance, then checks validity (`0xd4b58`–`0xd4b80`).
Custom OnAttach requires context, sync mode 0 and a media manager
(`0x86b38`–`0x86b6c`). Register stores manager/context and attaches each key
(`0x88984`–`0x889b8`); an identical existing manager short-circuits that work
(`0x88958`). For an ID-only attacher, Model's registerFile binds the integer
only when the context's coedit-mode requester exists and returns true
(`0x2b4530`–`0x2b4558`, named IsCoeditMode at `0x2ac2dc`). Otherwise, or after
failed Bind(int), the current integer remains unchanged. The actual requester
state for every WDoc open was not established.

The explicit `AttachFile(key,int)` API follows a different path (`0x88e50`,
`0x88f58`): Model requires a manager, releases prior ownership, calls Bind(int)
for non--1, then stores the requested ID without checking that Bind result
(`0x2b47ac`–`0x2b47e0`). Its returned bool reports manager availability,
rather than successful source resolution (`0x2b4844`–`0x2b4848`).

Custom validity requires context and manager (`0x86cb8`). Its attacher helper
accepts current ID `-1`; other IDs require metadata validity (`0x2b4c20`).
The resolved media helper, `0x293884`, rejects absent metadata or nonpositive
binding count, accepts flag `+57`, and otherwise requires its bool false,
manager coedit flag true and metadata flag `+58` false
(`0x293a40`–`0x293a84`). That body does not check disk existence.
`IsAllContentFileAvailable`, `0x870b0`, instead checks actual file existence
for non--1 current IDs, or nonnull deferred paths when context/manager is
unavailable (`0x87130`–`0x871a4`, media helper `0x28b504`). It skips `-1` or
null paths and can succeed with empty/skipped entries. Neither query alone
proves that every original saved reference resolved.

Custom Copy requires matching custom kinds (`0x86f54`), then overlays source
file keys onto the destination without clearing its file map (`0x89910`,
`0x89980`–`0x89a30`). Destination-only keys remain in that local body. Per-key
FileAttacher Copy uses the documented hash/path or deferred binding route,
rather than cloning the source numeric ID; the custom-level success does not
certify each binding. The inspected WPage, WPageImpl and Model PageImplBase
Copy bodies (`0xc90b8`, `0xd0264`, `0x346418`) do not directly traverse the
custom list or invoke this keyed Copy mechanism. This bounded observation does
not rule out cache rehydration, virtual callbacks or external custom-copy
handling, and does not prove app-wide loss.

Own-frame size and writing omit current-ID `-1` keys and patch the emitted
count (`0x8a28c`–`0x8a290`, `0x8a394`–`0x8a474`), without erasing runtime
nodes. Current `-1` is a state sentinel, not universally missing/deferred
bytes. Whole-custom admission is separate: loaded validity failure removes
the custom object, then operation bool false fails while true permits
continuation (`0xd4bb4`–`0xd4bf8`). Save checks validity before writing;
false policy fails, true permits omission of that entire custom record
(`0xd67e0`, `0xd68a8`–`0xd694c`). Save omission does not delete its runtime
node. These source-only contracts do not establish a real sticky archive
round trip, every deferred recovery, or whole-archive payload loss.

Rust bounds the page header and jumps to layers (`storage.rs:233–266`);
semantic flexible decoding reads only bits 0–9 (`page.rs:237–304`). It exposes
no owned custom-list records, validates no individual custom count/size, and
does not diagnose their kinds through layer-object warnings. Their bytes need
the original page buffer. High-level asset admission accepts only image
extensions (`container.rs:519–533`), omitting these `.sdocx` member bytes and
the custom key-to-ID relationship. Caller-owned archive bytes and browser
original ZIP retention are separate source carriers; original page bytes and
manifest resources do not become a typed custom-key/source-namespace graph.
Rust does not apply the native attachment or validity gates above. These are
source ownership boundaries, not verified archive loss or arbitrary
custom-vector semantics.

## Ownership differs among supported object families

| Family | Owned semantic or opaque data | Remaining source dependency |
| --- | --- | --- |
| Regular stroke | Decoded point, pressure, time, tilt and orientation arrays; known style, tool and named properties; undecoded style remainder; resolved pen strings | Common metadata beyond bounds, raw stroke masks and additional frames require the original record |
| Shape and line | Common metadata, geometry, control points, saved path bytes, known outline enums, signed pen references and unsupported paint payloads | Magnetic points, connection blocks and some fixed/flexible/style extensions are discarded; shape/line pen IDs need the original note string table for resolution |
| Page image | Bounds, rotation, selected main/border/original references, crop and original placement; accepted asset bytes | Complete inherited shape/fill, borders, coedit span data, unknown fields and frames require original payload; negative media IDs collapse to absent |
| Rich text | Original style/paragraph payloads and ranges; full embedded WDoc object bytes; typed image/table/code projections | Enclosing common metadata, text-common extensions and page-textbox border/extra frames are not completely owned |
| Formula, math and plot inspection | Explicit metadata APIs retain mapped fields, masks and remainder bytes; formula strokes and math formula envelopes own their embedded binaries | The inspection call requires original page bytes; these values are not automatically attached to the high-level page model or rendered |

[Accepted handwriting-to-text](stroke-recording-findings.md#handwriting-to-text-source-replacement)
can remove eligible current-layer originals before app text insertion; preserve original source separately from edited text.

The concrete decoders are
[stroke](../../crates/sdocx/src/decode.rs),
[stroke metadata](../../crates/sdocx/src/stroke_metadata.rs),
[shape/line](../../crates/sdocx/src/shape.rs),
[image](../../crates/sdocx/src/image.rs),
[rich text](../../crates/sdocx/src/note.rs),
[formula](../../crates/sdocx/src/formula.rs), and
[math/plot](../../crates/sdocx/src/math.rs).

Rich-text embedded objects have stronger byte retention than physical page
objects: `RichTextObjectSpan.object_data` owns its whole embedded WDoc binary,
including when `content` is absent. The typed projection and that original binary
are different representations. Table-cell constructor-default spans also
represent native load state rather than the exact saved span count.

[Physical Voice sources](voice-source-findings.md) retain separate audio/thumbnail
references in their own frames. Raw embedded/page source and note VoiceData
inspection do not supply a typed Voice attachment/session projection.

The formula [drawing contract](formula-rendering-findings.md#image-and-stroke-precedence)
uses a stored image when available and otherwise draws embedded source strokes,
then answer strokes. It does not evaluate LaTeX. Preserving those original stroke
records therefore does not require an expression evaluator. Image success and
stroke fallback must remain distinct to avoid drawing duplicate content.

Unsupported shape paint bytes are retained, including gradient payloads, while
[outline enums](shape-style-findings.md#saved-style-fields-and-loading) are typed.
This is stronger than their present drawing support. In contrast, decoded
[image effect settings](image-effects-findings.md), such as fill transparency and
nine-patch geometry, are not retained in `PlacedImage`. Keeping image pixels alone
does not preserve their clipping, transforms or effect placement.

## Resource identity and page paper

The [asset loader](../../crates/sdocx/src/container.rs) owns bytes for `media/`
entries ending in JPEG, PNG or WebP extensions. Other entries, including PDF
sources, are absent from `DocumentMetadata.media_assets`.

The [media manifest](../../crates/sdocx/src/media.rs) supplies authoritative
bindings while parsing. It is used by a transient resolver and is not returned
inside `ParsedDocument`; explicit manifest parsing is a separate API. A
`MediaAsset.archive_id` inferred from its filename prefix is not the same
identity as a manifest binding. A resolved array index also does not identify
immutable resource bytes when callers can replace assets.

The page decoder retains only the first PDF source-page index and discards
bindings, rectangles and additional entries. The
[PDF preservation constraints](../pdf-paper-preservation.md#current-rust-boundaries)
cover these losses, visible-page equality, original vector PDF transport and the
separate SVG conversion problem. Offset-only page retention applies equally to
non-PDF opaque vector records.

## Native opaque records, wrappers and resources

At a layer root, `LayerDocLoadHandler::ReadUnknownObject_WDoc`, Model
`0x359038`, constructs current type 19 and applies the original outer type and
binary through `ObjectUnknown::NewApplyUnknownBinary` (`0x359100`–`0x35912c`).
The [type-100 admission finding](brush-record-findings.md#type-15-is-a-compatibility-dispatch-envelope)
uses this route. Successful admission first requires a valid common base.
It then allocates and copies the **entire supplied original object binary**,
including that common base (`0x44fc48`–`0x44fc6c`), and separately stores original
type, binary length and original load format version (`0x44fc74`–`0x44fc88`).
This native ownership is stronger than the Rust physical index's external
byte-range reference.

The ordinary WDoc save path writes the object's current type, from `GetType()`
at Model `0x355460`, as the outer byte at `0x355490`. Unknown construction sets
that current type to 19 (`0x44f198`–`0x44f1a0`). `ObjectUnknown::NewGetBinary`,
`0x44f89c`, writes reconstructed current common data followed by an unknown own
frame; it does not return the original record wholesale. For a non-null original
binary without the optional string, `ObjectUnknownImpl::GetOwnBinary`,
`0x450984`, emits own-frame kind 19, original type/version, a four-byte original
length and the unchanged original bytes. For an original length `N`, the frame
occupies `N + 27` bytes; its flexible-data mask sets bit 1
(`0x4505bc`–`0x4505d8`).
Current outer type, own-frame kind, original outer type and original load version
therefore remain separate identities. Original byte preservation does not make
the reframed outer record byte-identical to its input.

A Rust harness using Unicorn's C API executed the actual native own-frame size,
writer and reader (`0x45093c`, `0x450984`, `0x450aec`), including native mask
parsing and bounded-copy logic. Four synthetic cases used original lengths
0, 1, 8 and 257; types 100, 27 and 255; and versions 7, 17 and `u32::MAX`.
Across allocation fills `00`, `a5` and `ff`, the wrappers and reloaded original
bytes/type/version were identical. Allocation, deallocation, memcpy and logging
were host boundaries. This proves the selected no-string own-frame kernel,
not complete original-record admission, resource reachability or archive saving.
The temporary capture SHA-256 is
`b1682308302f9807c7524e6d6c11e8a6e39fcc4b726574561b86aa15a73b6d6e`.

Opaque admission also depends on location. Layer-root out-of-mask records use
the unknown reader at Model `0x358728`–`0x358740` and join ordinary layer
insertion. `ReadObjectContainer_WDoc`, `0x358a74`, instead sends out-of-mask,
non-type-4 **children** to `SkipDefaultObject` at `0x358c24`–`0x358c48`.
Known children are appended, and type-4 containers recurse. Already wrapped
current type 19 is admitted by the known-type mask `0x01cfe58f`; an original
future-type child such as type 100 is a different case. This APK's skip branch
does not establish that the format forbids opaque children.

External child envelopes are separate from an opaque parent's copied bytes.
`Load_ObjectList_WDoc`, Model `0x358410`, reads the outer u16 child count for
versions at least 7 (`0x358518`–`0x358538`). Its successful default and unknown
readers receive no child count (`0x358584`, `0x358740`); only the type-4 reader
gets that count (`0x3586fc`, `0x358720`). `ReadUnknownObject_WDoc` reads a
four-byte length and exactly `N` own-payload bytes (`0x359090`, `0x3590cc`),
then supplies only that payload to the owned-copy route (`0x359114`–`0x35912c`).
Any integrity trailer inside `N` is included; the outer header and external
descendant records are excluded. Successful admission joins insertion at
`0x358590`, then advances the root loop at `0x3586dc` without child recursion.

If such a non-container input declares nonzero children physically following
its payload, the next root iteration reads the first child's type as a root
(`0x3586e4` → `0x3584b4`); if this is the final declared root, that loop stops
without consuming those records. This is a conditional source trace, not an
executed input, an observed valid-file loss or a format prohibition. The
inspected WDoc saver produces a different boundary: its non-type-4 branch
passes literal child count zero (`0x355554`, `0x355560`), which the default
writer emits as two bytes (`0x354fd0`–`0x354fd4`). Native opaque own-byte
preservation therefore does not establish ownership of an external subtree.

Rust's physical `storage.rs::parse_object` instead recurses over declared
children independently of object type, after consuming the parent's payload
and integrity trailer. `StoredObject.children` retains those ordered records
separately; `StoredObject::payload` borrows only the parent's payload from
caller-owned page bytes. Retaining that payload alone does not retain the
physical subtree. Original page bytes, child boundaries and integrity metadata
remain separate preservation inputs, without asserting that the APK renders
those descendants as the opaque parent's semantic children.

Already wrapped current type 19 can recover its original native type.
`ReadDefaultObject_WDoc`, Model `0x358d58`, first decodes the wrapper, then
retries its original binary/type when the stored original version is at most
5500 and its original binary is non-null (`0x358e2c`–`0x358ea0`). The retry uses
the enclosing loader's format-version argument, rather than substituting the
stored original version. Successful recovery returns the reconstructed object;
failed recovery retains the decoded wrapper (`0x358ea4`–`0x358ecc`,
`0x359010`–`0x359030`). This routine does not merge current wrapper common data
into the recovered original object. A type-19 envelope in source storage
therefore need not remain type 19 in runtime; automatic recovery also does not
establish preservation of the original envelope in a source model.

The unknown class inherits the inspected common rectangle/rotation setters
and `ReadyForSave`; its vtable resolves those entries to `ObjectBase`, rather
than unknown-specific synchronizers. The common kernels write rectangle,
rotation and modification time in current `BaseData` (`0x2d73c4`, `0x2d2bfc`,
`0x2cc574`). `ObjectUnknown::Copy`, `0x44fcfc`, likewise copies common state
through the base routine, then separately copies original type/version and
`memcpy`s original bytes (`0x44fd64`–`0x44fe20`). Current serialization writes
the current common frame followed by the original-byte own frame
(`0x44f8dc`–`0x44f8f0`). These inspected bodies do not merge changed current
common fields into embedded original common bytes. Context callbacks and
external pointer aliases remain separate possible routes. If the two common
representations differ without prior synchronization, successful automatic
recovery selects the embedded original representation. That conditional source
consequence is not an observed edit/save/recovery loss in an admitted document.

Original and current hash trailers remain separate. The raw unknown reader
passes the entire declared original binary to `NewApplyUnknownBinary`, without
subtracting its trailer (`0x359074`–`0x35912c`). Normal WDoc saving reconstructs
the current type-19 binary, then `WriteDefaultObject`, `0x354f78`, calculates
the current runtime UUID/time hash and appends 32 bytes after that new binary
(`0x3550c0`–`0x355174`). Thus an original trailer inside the copied opaque
bytes can coexist with a newly generated current trailer outside the wrapper.
Neither the new trailer nor the documented [logical integrity checks](integrity-findings.md)
authenticate the copied opaque payload. This is a static reader/writer contract,
not a complete archive round trip or proof of native integrity-policy behavior.

Media preservation has a separate gate. `MediaFileManagerNew::Bind(int)` and
`Release(int)` change metadata offset 24, the live binding count. `saveItem`,
Model `0x28fe5c`, admits nonzero-count resources after validation; zero-count
resources require the manager's coedit flag to be **true** and the operation's
fourth bool to be **false** (`0x28feb0`–`0x28fec8`). WDoc's `WNote::IsCoeditMode`,
`0x96a90`, reads WNoteImpl offset 854; `InitSubComponent` passes that same flag to the manager
(`0xa3844`–`0xa384c`, Model `0x28c2cc`). It is not an unknown-object flag.
For admitted entries, `saveItem` writes the existing numeric media ID and
filename (`0x28ff14`–`0x28ff58`), without rewriting IDs inside opaque binaries.
Its map-key field is copied, although the broader save path can refresh PDF
hashes before this iteration (`0x290458`–`0x29045c`).

`RemoveUnusedFiles`, Model `0x292cec`, additionally protects zero-count files
whose hash map key and numeric ID match the **previously saved** manifest
(`0x293464`–`0x29348c`). Its previous-entry parser skips the filename and builds
the map key from the 64-byte hash (`0x292ed8`–`0x292f7c`). A found key with a
different ID reports failure (`0x293614`–`0x293668`), rather than taking the
direct file-deletion branch. This deletion protection does not establish
inclusion in the next archive. Unknown's resolved original-version callback only lowers
the note's minimum unknown version (WDoc `0xa7bb8`–`0xa7bd0`); it does not bind
resources. The inspected unknown own-data paths contain no resource-ID scanner.
Decoded common image data can still register through `ObjectBase::OnAttach`
(Model `0x2d0018`). Owning opaque bytes and keeping admitted IDs stable do not
establish preservation of every attachment referenced only by opaque own data.

### Close-time working-cache cleanup

The app's `CloseDocumentTask.java:238,268` passes `true` to
`DocumentFileManager.closeWordDocument`; its forwarding lambda (`:89`–`:94`),
`SpenWordDocument.close` (`:83`–`:86`) and SDK `SpenWNote.close(removeCache)`
(`:1545`–`:1565`) carry that bool to WDoc JNI `WNote_close` (`0xe45b8`).
Native false callers also exist: `WNote::~WNote` (`0x8fd10`–`0x8fd14`) and the
[selected-object clipboard archive](#selected-object-clipboard-archive)'s
successful backup save followed by `Close(false)` (`0x96470`–`0x96480`).
These establish reached policies, without asserting every app caller or that
all temporary notes qualify for retention.

WDoc `WNoteImpl::Close`, `0xa39f0`, selects its retained-cache branch for a false
bool and a nonzero core-created timestamp (`0xa3a44`–`0xa3a4c`). The timestamp
is positively identified by `WNote::GetCreatedTime`, `0x96c08`, which reads
WNoteImpl offset 200. True or zero-timestamp closes instead remove the whole
working cache if present, checking `Directory::RemoveDirectory`'s result
(`0xa3b14`–`0xa3b28`). The deletion operand is the working cache path; these
branches do not establish an alias to, or executed deletion of, the original
input archive.

The retained branch first requires `DeleteInternalTempDirectory` to succeed
(`0xa3a50`–`0xa3a58`), then calls manager virtual slot 120
(`0xa3a5c`–`0xa3a68`). Model's installed `MediaFileManagerNew` vtable
`0x491a30` +16 resolves this slot at `0x491ab8` to `RemoveUnusedFiles`,
`0x292cec`. Its return is ignored before `SetCacheState(cachePath, 0)`
(`0xa3a6c`–`0xa3a74`). WDoc `WNote::Close` checks the outer close result before
WNoteImpl destruction (`0x8fd74`–`0x8fd88`); cleanup therefore uses counts before
that teardown, without establishing an atomic operation or complete deletion.

Ordinary save's successful tail separately calls `MediaFileManagerNew::OnSaved`
(WDoc `0xadb4c`–`0xadb50`), whose bounded Model method updates a manager timestamp
(`0x295fcc`–`0x295ff0`). That method does not call unused-file cleanup.
Retained-cache cleanup reads the working `mediaInfo.dat`, not the original ZIP;
its saved-key protection and current binding counts are separate from admission
to a later archive. A retained cache may already reflect earlier load, repair or
save. It is not automatically an exact original source carrier; Rust's supplied
original archive remains a distinct source unit.

### Copying between resource namespaces

Conflict merging supplies a concrete cross-context route:
`SyncConflictConfirmDialogPresenter.java:336–355` opens two note paths, saves
the sync note as a directory and opens a new merge note from it.
`CompareManager.java:516–521` assigns the other note as local;
`MergeNoteManager.java:46–55` copies its page into a newly inserted merge page.
The destination is seeded from the sync snapshot, so its existing resources
can overlap the source. This is not evidence of arbitrary empty-document paste.

WDoc `WPage::Copy`, `0xc90b8`, reaches `WPageImpl::Copy` and Model
`PageImplBase::Copy`, `0x346418`, then copies layers and their runtime objects.
The object loop factory-creates each source type and calls virtual Copy
(`0x35f13c`, `0x35f17c`), including current type 19: its factory branch
constructs Unknown at `0x36d7bc`. Fresh destination layers attach before copying
their objects (`0x34b10c`–`0x34b11c`). Their context clone shares the supplied
destination media-manager pointer at +416 (`0x2aa8a0`–`0x2aa8a4`); it does not
import the source manifest. Unknown inherits `ObjectBase::OnAttach`.

Current declared attachments have a separate transfer contract. Common copy
captures the source path (`0x2d8750`–`0x2d8758`) and passes it to AttachFile
(`0x2d83f0`). Under the ordinary attach gates, destination Bind(path) returns
the current ID stored in `BaseData+108` (`0x2cff4c`–`0x2cff54`). Bind can reuse
a destination hash match or allocate a destination-local ID
(`0x28c79c`, `0x28c86c`); the source number is not an input to that binding.
Known captured thumbnails separately use an ImageCommon local key, whose
ImageData stores the destination archive media ID (`0x2b86b8`–`0x2b86c0`).
These helpers do not receive or rewrite Unknown's embedded original bytes.

Copy success is not a complete transfer certificate. The whole-page object
loop ignores virtual Copy's result before adding its clone
(`0x35f17c`–`0x35f188`); common copy likewise ignores AttachFile's result.
Null context, missing manager or a true sync requester can bypass ordinary
binding. A pathless current ID is looked up in the destination manager,
without importing its source namespace. External listeners, adapters and
broader document operations remain separate boundaries; no private-reference
decoder or observed merge/save resource loss is established here.

Rebound current attachments are distinct from unchanged opaque bytes and their
originating resource namespace; those bytes alone do not establish portability.
Rust's archive parser builds its manifest resolver locally
([`container.rs`](../../crates/sdocx/src/container.rs)); `ParsedDocument` owns
neither that media manifest nor the original archive/page bytes. The
[external source-byte boundary](#original-page-bytes-are-external-to-the-parsed-model) also
limits recovery of opaque references and their original bindings.

### Selected-object clipboard archive

The selected-object clipboard path carries native objects alongside its HTML
appearance. `ObjectCopy.java:162–179` backs up the list to `.sdocx` before
generating HTML or a JPEG of selected strokes/math/plots; line 225 adds the
archive path in an HTML comment. That JPEG does not replace the preceding
native archive. `SpenWNote.java:1503–1507` throws on native backup failure.

The archive input is a processed selection. ObjectCopy adds shape followers
(`133–144`) and, for a single partially selected table, creates a typed table
copy and deletes rows/columns outside the selected row/column sets
(`234–288`). `TaskCopy.java:116–137` separately adds a PNG-backed image made
from retrieved selected PDF pixels; this helper does not transfer the PDF's
vector page. These are selection changes or derived image sources, rather
than a raw selected-record slice from the original archive.

WDoc `WNote::BackupObjectList`, `0x961b0`, creates a temporary note and one
page (`0x963cc`, `0x9643c`), then checks the utility copy and Save results
(`0x96458`–`0x96474`). The true Save branch reaches actual `NoteZip::Zip`
(`0xad630`–`0xad664`). `WNoteUtil::BackupObjectList`, `0xba770`, creates each
admitted runtime type and checks virtual Copy and page append
(`0xba850`, `0xba8b4`–`0xba8b8`, `0xba9b4`–`0xba9b8`). This saves cloned
editable objects without establishing original wire-record byte identity.

The utility skips PDF dummy objects (`0xba814`–`0xba840`). For source type 2,
it removes timestamp spans from the **source** ComponentText before cloning
(`0xba894`–`0xba89c`): filter `0x80000` matches
`SpenTextSpanBase.java:23`, `FILTER_SPAN_TIME_STAMP`. This is metadata removal,
not proof of removed text content or pen geometry. Source owner-page offsets
are added to the clone's RectF (`0xba8c0`–`0xba91c`), with separate integer
image OriginalRect and double painting OriginalRect adjustments
(`0xba930`–`0xba9a0`). Runtime-handle mapping can also rewrite connected shape
information (`0xba9c0`–`0xbaa0c`). These editing conversions are separate from
the precision of the original saved coordinates.

`WNote::RestoreObjectList`, `0x96648`, loads the temporary archive and checks
its utility restore (`0x967d4`, `0x96858`–`0x9685c`). The utility checks page
loading, factory Copy and output-list Add (`0xbabbc`, `0xbac88`–`0xbac8c`,
`0xbad90`–`0xbad94`). Type 19 can be copied if it remains the runtime type
after loading; automatic recovery can instead produce its original known type.
For types 2/7, nonzero archived-minus-current body-font delta adjusts float
font-size spans (`0x96804`–`0x9681c`, `0xbacd4`–`0xbad48`). Original byte
identity and physical font-program transport do not follow from this copy.

Restore returns a list in the caller note implementation at +264
(`0x96848`), before destination-layer insertion. `TaskPasteHandler.java:90–132`
chooses PasteObject for an existing native selected-object path when plain-text
paste is disabled; whole-page clipboard data uses a separate route.
`PasteObject.java:40–68` restores and applies restrictions before
`ObjectManager.java:233–305,641–652` inserts surviving objects through page or
position policies. Clipboard placement depends on those insertion policies.
The temporary note is closed before return (`0x96868`); resource-path lifetime
depends on cache state and later attachment. Declared attachments use the
[destination binding contract](#copying-between-resource-namespaces), while
unchanged opaque bytes do not certify transport of private resource references.
The native clipboard archive therefore preserves an editable representation
separately from original archive bytes and external HTML/JPEG appearance.

### App page copy carries a separate body-text envelope

`TaskCopyPage.java:91–98` calls `NoteManager.copyPage`; its JPEG `1.jpg` and
HTML backup-path comment are separate (`61–70`). `NoteManager.java:249–280`
constructs a temporary LIST note and walks the supplied selection list in its
current order, skipping zero-width/height pages. For each eligible page it
copies that page, then appends another same-sized physical page containing a
fresh container. The container has integer `page_copy_bookmark_key` and a first
textbox child from `copyBodyTextFrom(sourceIndex)` when nonnull; it remains
present without that child. The temporary note records the source body-font-size
delta. This is an app clipboard schema, not a rule for interpreting every
two-page LIST note.

The producer calls `saveAsDirectory` despite naming the path `.sdocx`
(`SpenWNote.java:2394–2411`). Native false enters SDK error handling; the
file-manager wrapper propagates action exceptions. The name alone does not
establish ZIP output, and the JPEG does not replace the editable native source.
The [native save boundary](#native-page-files-during-save) still applies.

WDoc `WNote::CopyPage` (`0x93af4`) reaches manager CopyPage (`0xb50ac`), which
constructs a new source-sized/oriented page with no supplied UUID
(`0xb5120–0xb5154`), calls `WPage::Copy(source,true)` before destination
insertion (`0xb5168`, `0xb5198`), and validates index in `[0,count]`. Model
Construct (`0x34367c`) generates the UUID; WPageImpl starts with null context
(`0xcd42c`). The inspected copy kernels do not replace that UUID with the
source UUID: Model Copy (`0x346418`) leaves member `+48` untouched, CopyTag
(`0x344b98`) edits the separate tag list, and cache Copy (`0xd05e0`) reads both
UUIDs as filenames. Preattachment coedit notification exits on null context
(`0xce920–0xce924`). This is a copy-phase identity result, not a claim about
later attachment callbacks or save/reload identity.

WPage Copy checks source loading and implementation copy, but ignores PDF-copy
and template-type returns (`0xc9258`, `0xc9264`) before returning true. PDF
copy carries source path, page index and rectangle; destination binding is
separate and requires a media manager absent during this preattachment phase.
Insertion adds the pointer before void OnAttach/callback/Bind
(`0xb7da4–0xb7db8`); OnAttach can return early or skip custom attachment after
failed media attachment (`0xceec4–0xceec8`). Success is not a complete asset
certificate. Existing [copy/resource limits](#copying-between-resource-namespaces)
and [custom-object limits](#loaded-custom-file-maps) remain separate.

Composer `CopyBodyTextFrom` (`0x3a1cbc`) reaches Bodytext CopyPage (`0xbc214`)
and copyPages (`0xbbd24`), creating a fresh textbox from selected body sections.
The measured-range route asks `GetTextSectionResult` (`0xc17ec`) then copies
that result (`0xc180c`). BodyTextUtil (`0xcdbfc`) dispatches to Widget CopyText,
or only CopyFontSizeSpan when its source-content flag is false. Widget passes
inclusive end−1 to Model CopyText (`0xdf318–0xdf358`). Model TextCommonImpl Copy
(`0x3ec1f0`) copies the UTF-16 substring, margin/gravity, spans and paragraphs;
it is not a concatenation of page object lists.

ObjectSpan range copying includes anchors start≤index≤end and rebases them
by −start (`0x3fd34c–0x3fd384`). Its source-object copier creates the actual
source type, calls virtual Copy and binds that fresh instance
(`0x417a54–0x417a9c`), preserving an embedded-source route beyond thumbnails.
That virtual-copy return and the textbox wrapper's return are unchecked
(`0x417a78`, `0xbc31c`); arbitrary child/resource success or universal saved-UUID
rewriting does not follow from fresh-instance construction.

The derived slice removes timestamp spans, filter `0x80000` (`0xc0b60`),
synthesizes size 17 over nonempty text only when font-size spans are absent
(`0xc2238–0xc2270`), carries a positive boundary line-height font size
(`0xc22b4–0xc22d4`), and can fill page boundaries with blanks. Those editing
normalizations are distinct from original saved strings/styles/source bytes.

`PastePage.java:136–184` consumes copied page `i` plus companion `i+1`, reads
the container's first textbox/bookmark metadata and advances by two. It checks
`i < pageCount`, without proving admission of arbitrary malformed pairs. It
rejects coedit notes, mismatched orientation, destination SINGLE mode,
template type 17 and the applicable text limit; already-added pages can still
be committed if a later selection fails. Bookmark insertion uses the new page
ID. `NoteManager.java:873–896` locks body text, copies the page, replaces its
background with the destination background, requests layout, then reinserts
the carrier using destination ratio and transported font delta, or an empty
body-text section.
Composer's manager (`0x3a1e04`) sends normal modes to InsertPage; mode 1 uses
cursor 0/new-paragraph true. Its inspected switch does not directly use the
supplied height/ratio. Bodytext adjustment subtracts the current editor delta
from the incoming delta and removes timestamp spans again (`0xbed54–0xbee0c`).
Source transport therefore does not certify identical pasted appearance.

Rust retains physical page order/UUIDs, typed container children and document
font delta. Common UUID and ExtraData are inspectable through ObjectMetadata
and ObjectFlexibleMetadata, but semantic PageObject containers do not retain
the bookmark bundle as a dedicated field. The parser is not this clipboard
mutator; this trace does not justify dropping companion pages from ordinary
notes or assert a complete native save/reload, asset or appearance result.

### SPD to WDoc migration

The app's legacy migration is another editable-source route.
`SPDToSDocXConverterWrapper.java:130–164` opens a SpenNoteDoc with the provided
WNote's page-default width, then calls `SPDToSDocXConverter.java:185–226`.
That converter uses the **loaded** source note/page dimensions as destination
defaults and page sizes. The old loader's width normalization and original SPD
header identity remain unestablished. It also appends a final page (`274–278`).

`MultiPageNoteComposer.java:167–173` supplies scale 1.0 and PointF(0,0) to
transferObjects; `SpenWPage.java:1186–1194` throws on false. WDoc's
`WPage::TransferObjects`, `0xc9c78`, passes the PageDoc's **current-layer cookie**
(`0xc9ce8`) to Model `PageImplBase::TransferObjects`, `0x345550`. Its handler
route reaches `ObjectManager::TransferObjects`, `0x35f404`. This inspected
operation does not establish migration of every physical source layer.

The manager transfers existing runtime pointers. Each object passes through
GetRect, temporary RectF scale/offset, OnTransfer(destination context),
SetRect(false,true) and destination-list Add (`0x35f4a8`–`0x35f52c`), without
factory cloning or a stroke-array reconstruction loop. SetRect/Add results
are unchecked. Source-list RemoveAll (`0x35f564`) clears membership nodes:
Base List Add stores the pointer at node+16 (`0x9d01c`–`0x9d030`), while
RemoveAll frees only nodes (`0x9e30c`–`0x9e330`). Conditional on successful
destination additions, clearing the source list does not dispose its objects.

Neutral parameters still run stroke rectangle methods. SetRect returns before
point mutation when requested/current common rectangles compare equal
(`0x2de15c`–`0x2de180`). However GetRect expands zero width/height by one unit
(`0x2e6b28`–`0x2e6b44`), so scale 1/offset 0 alone does not prove equality.
Unequal rectangles can invoke ApplyRect on the PointF vector
(`0x2de184`–`0x2de2b4`, `0x2e8f60`–`0x2e9070`). Stroke OnTransfer separately
rebinds pen-name/advanced-setting string IDs (`0x2e69c0`–`0x2e6a7c`); its
inspected body does not replace the implementation or rebuild the other
stroke-channel arrays. Neither boundary certifies original wire precision.

The actual Magic Pen converter runs after transfer. Its admission requires
type 1, a SpenObjectStroke instance and the exact name
`com.samsung.android.sdk.pen.pen.preload.MagicPen`
(`SPDToSDocXConverter.java:167–173,229–249`, `SpenPenManager.java:43`).
`HandWritingConverter.java:16–24` calls setColor with
`(destinationBackground & 0x00ffffff) | (strokeColor & 0xff000000)`:
current destination-page RGB replaces stroke RGB while source alpha remains.
That background is read before the later page-background assignment
(`SPDToSDocXConverter.java:260–265`); no fixed white/dark value is inferred.
The native kernel writes saved color member 288 (`0x2ea388`) and marks
changed/cache-dirty, without rewriting point arrays in that local kernel.
Converter status handling does not certify that every selected rewrite succeeds.

Base OnTransfer resolves the declared BaseData+108 attachment ID through its
old manager and rebinds its path (`0x2d02dc`–`0x2d0324`); ImageCommon transfers
its ImageData owners (`0x2d03c8`, `0x2b7034`). Those lookup/bind results are not
individually checked. Destination context/dimension state is then installed
(`0x2d03cc`–`0x2d040c`). These are declared-owner routes, separate from a whole
manifest import or references inside opaque original bytes.
The resulting WDoc serializes migrated runtime state. Its saved coordinates,
channels and ARGB are converted source, without evidence for another
extension-based DPI transform or reversing the admitted color rewrite.
These static routes do not establish original SPD archive preservation,
complete channel/resource equivalence or observed original-vector loss.

### SDOC content conversion

The reached app envelope differs from the [whole-SPD transfer](#spd-to-wdoc-migration).
`ConvertServiceManager.java:374–388` gets the queue destination SpenWordDocument
and selects `SDocXConverterWrapper` when no custom converter is supplied. Its
runner calls conversion before saving (`:146–163`); repository construction
chooses a supplied path or one ending `.sdocx`, plus destination UUID/page mode
(`:394–409`). `converter/sdoc/SDocXConverterWrapper.java:68–103` opens a SpenSDoc
and calls `sdoc/model/SDocXConverter`, which iterates content blocks into a
SinglePageNoteComposer (`:122–146`), rather than importing an entire archive.

The dispatcher installs handwriting and Drawing converters separately
(`SDocXConverter.java:69–91`). `runConverter`, `:111–119`, discards the converter
boolean; absent types only log. `AbsContentConverter.java:16–25` also ignores
convertContent's boolean and returns true unless it catches an Exception.
Therefore outer conversion/save success does not certify each content's
admission. Null source creation can make the void wrapper return early.
No unsupported-content log here establishes observed vector or resource loss.

SDOC `sdoc/model/converters/HandWritingConverter.java:387–423` requires a nonempty
attachment, initializes its NoteDoc, then selects only `getPage(0)` for composition.
This is not evidence that any actual attachment contains omitted extra pages.
Its native loader is positively reached: SDOC JNI `0xd7e14 → 0xd7e40` calls
`ContentHandWriting::CreateNoteDoc`, `0xb784c`. It reuses cached impl+160 or gets
the attached path (`0xb7964`). A successful getter/nonempty path calls old
`NoteDoc::Construct(app-dir, path, null, supplied-width, mode=1, false)`
(`0xb79e8–0xb7a04`), without a thumbnail decoder. Failed getter/empty returned
path takes fresh width/height construction; a nonempty missing filesystem path
still enters the loaded constructor. Failure clears the cached pointer.
Complete old-file channel/resource admission and width normalization are unproved.

Its container helper (`HandWritingConverter.java:163–195`) recursively visits
containers, converts type-2/7 shapes and collects type-2 children unless parent
extra integer `Type` equals 23. A nonempty collected list triggers child removal/
page append; an empty container is removed. Admitted nesting can therefore change.
Shape rect/style and layout setters are also present; the top-level decompiled
cast/fallthrough is not treated as universal per-type dispatch. Background image
conversion (`:140–160`) removes page objects before capture, restoring the list
only inside the nonnull-capture-creator branch. Failure restoration/instance
lifetime are not established by that body. Background color creates new fill
rectangles and uses the previously described MagicPen RGB rewrite.

`SinglePageNoteComposer.java:202–210,329–337` clears source history and supplies
scale 1, PointF(`getMDP()*24`, accumulated height) to WPage.transferObjects.
This caller adds layout placement to the [current-layer pointer transfer](#spd-to-wdoc-migration),
whose rectangle methods can mutate coordinates; do not reverse that placement
or reconstruct earlier channels from modern saved geometry. The composer derives
new first-page height/background (`:272–292`). Extra pages/layers/media and
numerical migration parity remain unexecuted, not declared lost by these calls.

`DrawingConverter.java:23–76` requires an attached source and thumbnail. Its
nonnull display-metrics branch creates ObjectPainting with thumbnail/layout
and passes the attached source to composer.appendObject; `:322–326` calls
object.attachFile before append. These are distinct [Painting source and cache](painting-source-findings.md)
carriers. Null metrics has no append in the decompiled branch yet returns true;
r3/z decompiler artifacts prevent an unconditional runtime branch guarantee.
The PaintingDoc and loaded handwriting NoteDoc are closed afterward; neither
source-file lifetime nor unchanged original Painting bytes follow from that alone.

The explicit-destination save route is concrete: ConvertRunner.saveTempFile
(`:107–114`) → NotesDocument.saveDocument (`:672–673`) → WordDocRepository
(`:438–451`) → DocumentFileManager.saveWordDocument/lambda (`:225`, `:123–126`)
→ SpenWordDocument.save (`:195–199`) → saveAsDirectory. WDoc JNI `0xe61f4`
passes file-output false (`0xe6288–0xe6290`) to the
[working-cache saver](#native-page-files-during-save). The separate normal-service
save call is not replaced with this explicit-path implementation. A `.sdocx`
path suffix or SaveResult does not prove final ZIP publication or full conversion.

Source cleanup is a separate call boundary. `SDocXConvertTask.java:47–66` attempts
FileUtils.deleteFile for source marked outside DB even when conversion-success
argument is false; `onFailed`, `:77–84`, can call it when the failure argument is not 3.
DB-managed DeleteNoteUseCase additionally requires enabled-delete flag, successful
message, destination without `/converted/`, source UUID and nonempty result.
These guards do not prove actual deletion, successful backup or preserved originals.
Wrapper postprocess attempts old lock-file attachment then closes source SDOC
(`:41–55`), without establishing a whole original archive attached to WNote.

Rust parses modern records rather than this legacy content dispatcher. Keep
original source independently of admitted modern channels and Painting attachments;
image-only media admission, outer save success and runtime pointer migration
cannot replace missing source bytes or justify a second migration renderer.

## Precision and drawable output

Saved WDoc points, rectangles and path bytes can contain `f64` values.
[Native stroke writing and restoration](object-transform-findings.md#compressed-format-loss-and-native-restoration-precision-are-separate)
distinguish compressed-format loss, runtime float arithmetic and Rust's retained
decoded values. Native drawing reconstruction has further
[profile-specific arithmetic](stroke-rendering-findings.md#shared-preparation-and-replay).
Source precision and drawable precision are separate boundaries.

The [SVG adapter](../../crates/sdocx/src/render/vector/path.rs) uses the library's
`f32` path parameters. Its `coordinate` helper first formats the requested
number of decimal places, then parses that value as `f32`.
[`native_svg_path`](../../crates/sdocx/src/render.rs) requests two decimal places
for saved shape/line paths. A `0.004`-unit displacement can consequently become
`0.00` before narrowing, even within the finite `f32` range. The owned native path
bytes remain unchanged. Other routes have different policies: basic shape/line
attributes and generic ink commonly use two places; stamp paths commonly use
four; some attributes accept validated `f64` values without fixed formatting.

This output quantization is an approximation boundary, not proof of an incorrect
native arithmetic port. Full saved numerical identity does not survive merely
because the emitted element is vector. The
[SVG serialization contract](../svg-rendering.md#validation-and-serialization)
already records these precision and validation rules.

Arc/oval native path commands retain their source bytes but reject the entire
unsupported drawable path. Invalid or unrepresentable SVG elements are omitted;
invalid containers skip their children. The adapter itself has no source-aware
omission report. Retained PDF glyph transport checks its own registry and reports
missing or unsupported retained text as export errors. That text safeguard does
not certify geometry omitted before SVG carrier parsing. Generic imported SVG
filters can also introduce rasterization; that separate compatibility output is
bounded in the [output support table](../rendering-support.md#output-and-verification).

Source inspection identifies a separate image-omission boundary in PDF export.
Archive image admission owns bytes by filename extension without decoding them.
The [PDF resolver](../../crates/sdocx/src/pdf.rs) validates PNG, but unreadable
JPEG/WebP dimensions can make usvg omit an image node without setting the SDK's
`InvalidImage` error. The bundled [image converter](../../crates/sdocx/src/pdf/svg/image.rs)
also turns image-constructor rejection into `None`, whose
[caller](../../crates/sdocx/src/pdf/svg/group.rs) ignores the result. These are
source-observed rejection paths, without an executed malformed-image probe or
evidence that a valid corpus image was dropped. Successful PDF generation alone
does not verify transport of every retained image; original image bytes remain
a separate preservation boundary.

## Diagnostic interpretation

Current parser messages describe unsupported known objects as “payload retained”
and unknown types as “retained unknown”. For physical page objects, this means
type and structural location are retained; it does not mean the returned model
owns the payload. Image/shape diagnostic categories likewise cover selective
semantic or opaque data rather than every source field.

An empty parse report is not a preservation certificate. Inactive objects are
not semantically decoded; regular stroke extra frames are validated and dropped
without a dedicated warning; some note/title/nested-text helpers discard their
unsupported-feature list; and invalid drawable elements may be omitted without
an output diagnostic. Conversely, an unsupported-feature warning does not mean
all original vector data was lost: embedded WDoc binaries, shape paths,
unsupported paints or retained source assets may still be available.

The browser's diagnostic transport is narrower than Rust's render result.
[`DocumentSession`](../../crates/sdocx-wasm/src/lib.rs) returns only `page.svg`
for preview and `output.bytes` for PDF export, dropping the render-produced
text/object diagnostics. Its inspection report contains `parsed.report`;
the [web inspection view](../../web/src/lib/converter/view-model.ts) displays
those parser diagnostics. Debugger background/replay requests also return only
the SVG and default ink color. Original source inspection remains available,
but these routes do not expose render fallback reports. The
[CLI](../../crates/sdocx-cli/src/main.rs) separately reports render diagnostics.
