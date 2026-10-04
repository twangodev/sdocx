# Physical Voice sources and attachment identity

## Evidence boundary

These static Java/ARM64 traces in Samsung Notes 4.4.45.37 establish no executed
archive round trip or observed source loss.
Addresses are ELF virtual addresses in the named library. Java paths below are
relative to `scratch/apk-analysis-decompiled/sources/com/samsung/android/support/senl/nt/`.
APK and Model/Composer hashes match the [card-source evidence](card-source-findings.md#evidence-boundary).
WDoc SHA-256 is `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6`.

## Reached page and span source

`OptionMenuAudioFilePresenter.handleAudioAddResult` distinguishes the same
downloaded-audio source: `!z` calls `insertObjectVoice`, while `z` calls
`insertVoiceData` (`composer/main/base/presenter/menu/option/OptionMenuAudioFilePresenter.java:101–139`).
`ObjectManager.insertObjectVoice` constructs physical type 10, sets title,
state 4 and attachedFile, then validates adding before insertion. Outside coedit
its style setter supplies version 3, viewType 1, gradients and dummy colors
(`composer/main/base/model/composer/ObjectManager.java:116–124,187–195,654–667`).
`ObjectInserterNormal.java:83–85` inserts into a page;
`ObjectInserterObjectSpan.java:172–174` inserts a text object span. These differ
from [note VoiceData](note-metadata-findings.md#sized-records) and
[voice synchronization](stroke-recording-findings.md#voice-synchronization-uses-append-time-and-original-objects).

Model `LayerDocSaveHandler::WriteDefaultObject` `0x354f78` selects modern
size/writer slots 392/400 in its non-compatible WDoc branch (`0x354fd8–0x354fe0`,
`0x355078`, `0x3550a8`). Voice vtable entries `0x498240`/`0x498248` resolve to
`NewGetBinarySize` `0x4370d8` and `NewGetBinary` `0x437148`. Buffer-check
failure or a negative writer result fails this save path; dispatch is not an
executed successful archive. The [ObjectSpan WDoc pair](text-box-findings.md#native-textcommon-object-span-trailer)
`0x417be4`/`0x417c98` retains the complete embedded WDoc object binary and
UTF-16 index/placement fields, rather than a separate Voice timing record.

## Own frame and resource bindings

Model `ObjectVoice::NewGetBinary` writes ObjectBase then calls Voice's own
writer (`0x43719c` -> `ObjectVoiceImpl::GetOwnBinary` `0x43ac78`). Its 15-byte
header contains type 10, the recorded byte and a u16 flexible mask. The matching
modern loader `0x437230` loads ObjectBase and checks remaining length before
`ApplyOwnBinary` (`0x437318`); the own reader checks type 10 (`0x43adcc`), loads
the recorded byte (`0x43ae68`) and invokes the flexible reader (`0x43ae88`).
It sets runtime state 4 on its accepted exit (`0x43ae90–0x43ae94`); state 4 is
not an additional saved own-frame state field in this pair.

Flexible bit 0 is i32 audio media ID at impl+104 (`0x439b0c–0x439b60`);
bit 6 is i32 thumbnail media ID at impl+296 (`0x439d54–0x439dac`). Each is
omitted at -1; checked reading stores them at `0x43a0a0` and `0x43a1e8`.
Title/play-time/body strings, gradients, version, colors and viewType are also
own fields, distinct from measured card geometry. The inspected own writer
emits no additional action/session list matching note bit 13 VoiceData.

The two IDs belong to FileAttachers at VoiceImpl+32/+224, whose ID member is
+72. Audio setter `0x43a558` and thumbnail setter `0x43b214` release/bind
their corresponding resources. Physical OnAttach `0x4373b8` reaches Register
`0x439864`, which attaches both (`0x4398e4`, `0x4398f0`), subject to its
context/service gates and unchanged-manager shortcut. The established
[FileAttacher and media gates](card-source-findings.md#bound-resources-have-consumers-beyond-card-drawing)
apply: own IDs are neither inline audio bytes nor unconditional archive inclusion.

## Copy and coedit reference transitions

The app clone dispatcher constructs Voice for type 10 and calls `copy(src)`
(`composer/main/compare/clone_object/CloneSpenObject.java:30–35`). Model
`ObjectVoice::Copy` `0x438754` checks kind 10 and base/history gates, then calls VoiceImpl copy (`0x438ad0`):
FileAttacher::Copy separately for audio/thumbnail (`0x43a9bc`, `0x43aa38`).
The shared copy `0x2b48ec` tries a nonempty source hash and AttachFileHash,
otherwise releases/rebinds the destination from the source path
(`0x2b492c–0x2b4978`); a detached fallback retains hash/path strings
(`0x2b49a8–0x2b49d0`). This is not blind numeric-ID copying or guaranteed byte copying.

`VoiceRecordingEventListenerImpl.setCoedit` selects `CoeditRecordingEventAction`
(`585–588`). Its `updateRecordStopState` (`620–647`) inserts physical Voice
using VoiceData's attachedFile, then removes the old VoiceData. The insert
Boolean is ignored; ordering does not establish successful insertion.
`VoiceModel.java:450–464` removes the matching handle and calls WNote.removeVoiceData.
WDoc JNI `0xec10c` -> WNote::RemoveVoiceData `0x99f78` -> WNoteImpl `0xa6154`
removes its list entry (`0xa61e0`), invokes OnDetach (`0xa61f4`, slot 24 resolved
at `0x103440`), notifies an event callback (`0xa6214`), then releases the
VoiceData instance (`0xa621c` -> `0xd8d70`).

WDoc VoiceData::OnDetach `0x8da40` skips implementation detach for null context
or nonzero ModelContext.GetSyncMode. Its zero-sync branch reaches `0x8e684`,
resolves the manager path (`0x8e6f8`, slot 80), releases the ID (`0x8e70c`,
slot 72), then sets the ID to -1 (`0x8e714`). Model slot 72 is
MediaFileManagerNew::Release(int) `0x28f084`: it decrements a positive live
count, without file unlink/map erasure in this inspected method. Bind(path)
`0x28c5dc` computes FileHash/Get; an existing-hash branch increments the count
and returns the existing ID (`0x28c774–0x28c79c`). The same manager and matching
hash can therefore reuse identity before release. Equal paths, unchecked insertion
or callback ordering do not prove that route or whole resource lifetime preservation.

## Consumers and current Rust ownership

Composer playback setup prefers physical Voice's attachedFile when present
(`NoteVoiceManager::RequestInit` `0x441b10`, `RequestPlay` `0x441bf0`;
`0x441b98`/`0x441c78`), otherwise VoiceData's (`0x441ba8`/`0x441c88`).
`office/powerpoint/PowerPointView.java:403–420` dispatches type 10 to
VoicePPTController, which resolves its attachment and adds an audio PackagePart
and timing relation (`VoicePPTController.java:66–80`). The inspected
`VoiceUtils.writeFile:157–180` creates/reuses a part; final audio-byte copy,
extension rejection policy and successful export are not established by that helper.

Rust StoredObject retains page payload identity and requires original page bytes
to borrow it. RichTextObjectSpan.object_data owns the embedded WDoc bytes even
without typed content. Neither is a Voice own-frame decoder; typed NoteVoice
decodes the separate note envelope. Original media, raw page/span bytes and the
high-level semantic model remain [different source carriers](vector-retention-findings.md#ownership-differs-among-supported-object-families).
Actual manager/hash aliasing, sync mode, callback/instance lifetime and successful
save/reload with media remain bounded gaps; no original-byte deletion is inferred.
