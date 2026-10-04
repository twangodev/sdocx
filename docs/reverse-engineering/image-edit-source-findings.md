# Image editing changes current source separately from original bindings

## Evidence and scope

These are static ARM64 and decompiled Java findings from Samsung Notes 4.4.45.37,
APK SHA-256 `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
ObjectControl (1,503,872 bytes) and View (676,840 bytes) match their pinned APK members
byte-for-byte. Addresses are ELF virtual addresses in the named library, defaulting
to Model. No native editing, generated archive, history roundtrip or appearance comparison is claimed.

| Library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenObjectControl.so` | `3211b70ad105e285b57aaa085e2bcd543f197238d702f9233aedea1c7caa1aea` |
| `libSPenView.so` | `c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8` |

The [saved image fields](image-findings.md) and
[rectangle/cache setters](image-effects-findings.md#crop-editing-updates-separate-rectangles-and-invalidates-cached-pixels)
are separate from the application source replacement routes below. Java composer
paths are relative to `sources/com/samsung/android/support/senl/nt/composer/main/base/`
in the identified APK decompilation unless another prefix is stated.

## External editor preparation uses current main pixels

`presenter/menu/contextmenu/ContextMenuBuilder.java:235` installs the photo editor.
`ContextMenuPhotoEditor.java:241–259` obtains the image and prepares TaskPhotoEditor
under its readiness/permission gates; `:183–208` starts the external photo/AGIF
editor with the prepared URI and `notes_objectId`. That external implementation
is outside this APK trace. `presenter/task/TaskPhotoEditor.java:64–77` uses
HandleImage.saveAsImage, then requires a nonempty existing output file.
`presenter/share/HandleImage.java:51–61` delegates to
`sources/com/samsung/android/support/senl/nt/base/composer/share/BaseHandleImage.java`.

For type-3 images, BaseHandleImage `:89–98` reads the current main path/bitmap,
crop rectangle and separate original-image path. Its admitted crop operates on
that current bitmap (`:54–66`, `:131`); the original path is not selected as the
bitmap input here. `:132–165` copies the current file for GIF regardless of crop,
or JPG with null/empty crop. Other branches encode the cropped/current bitmap
with PNG and quality 100, including a branch whose requested suffix is JPG.
It subsequently passes the output, current path, original path and crop to
markGenerativeEditImage (`:168`). This trace does not establish that callback's
byte effects. Copy IOException is logged and the bitmap-save result is ignored;
the task's file-existence check is a separate admission, not decode validation.

## Returned editor source conditionally clears the original binding

`presenter/menu/contextmenu/ContextMenuDispatcher.java:75–78` dispatches the
ImageEditor result. ContextMenuPhotoEditor.onActivityResult `:280–317` requires
result code -1, nonnull Intent/output, an existing output file, and the object
located by `notes_objectId` to be a type-3 SpenObjectImage. The existence helper,
`sources/androidx/activity/a.java:12–13`, is `new File(path).exists()`.

The handler records **nonempty original-image path AND nonnull OriginalRect**
(`:307`), without testing rectangle emptiness. It clears crop, clears OriginalRect,
and calls setImage(output) in order (`:308–310`). Only when that recorded condition
is true does it call setOriginalImage(null) (`:313`). A nonempty original binding
without OriginalRect therefore does not trigger that clear call. commitHistory
and temporary-directory cleanup follow (`:315–316`); this body has no transaction
or rollback proving complete mutation, undo preservation or archive publication.

`sources/com/samsung/android/sdk/pen/document/SpenObjectImage.java:376–380` and
`:342–346` call the String image/original JNI methods and throw on false. Model
ObjectImage_OnLoad `0x420f58` registers table `0x4b50f0` (`0x420f90–0x420fa0`);
entries `0x4b5390` and `0x4b54b0` pair those names/signatures with relocations
`0x4b53a0` → ObjectImage_setImage2 `0x4231e8` and `0x4b54c0` →
ObjectImage_setOriginalImage `0x423c94`. The former forwards to ComponentImage
String SetImage at `0x423290`/`0x4232e4`; the latter forwards String or null to
SetOriginalImage at `0x423d34–0x423d3c`.

ComponentImage::SetImage(String*, Rect), `0x3a17dc`, constructs a **fresh temporary
FillImageEffect** (`0x3a1878`). SetImageUri (`0x3a18c8`) must return true before
live ObjectShapeData::SetFillEffect (`0x3a18e4`); the live setter's bool propagates,
while its separate changed byte controls cache clearing (`0x3a18f0–0x3a18f8`).
SetImageUri `0x3b8604` stores its AddImage index and returns whether it is
nonnegative (`0x3b86e4–0x3b86f4`); null path admits an empty effect (`0x3b86fc`).
Temporary admission does not prove removal of old live main before a failed path.

SetOriginalImage `0x3a1b58` manages a distinct internal index at +180. Admitted
branches remove its prior ImageCommon entry and set -1 (`0x3a1c84–0x3a1c90`,
`0x3a1d48–0x3a1d54`); nonnull path adds another resource (`0x3a1cac`, `0x3a1d78`),
while null skips AddImage. History creation/submission can fail (`0x3a1c54`,
`0x3a1e18–0x3a1e30`). Reference removal is not proof of physical file deletion.

## Native lasso reaches current-source replacement

`presenter/menu/contextmenu/ContextMenuLassoCrop.java:50–51` calls
ObjectManager.setLassoCrop(true); `model/composer/ObjectManager.java:799–800`
forwards to `sources/com/samsung/android/sdk/pen/control/SpenControlObjectManager.java:479–483`.
ObjectControl Control_OnLoad `0xd5390` registers that SDK class/table `0x1767d8`
(`0xd53cc–0xd53f4`); Native_setLassoCrop `(JZ)V` entry relocation `0x176890`
resolves to `0xd594c`, reaching ControlObjectView::SetLassoCrop `0xfdb20`.
It stores enabled property +796 (`0xfdb80`). The actual parent constructor makes
ControlLassoCropView (`0xfa0ec`), stores child +1288 (`0xfa0f4`) and adds it to
its ViewGroup (`0xfa128`).

ObjectControl DispatchTouch `0xfd450` → preDispatchTouch `0xfcf34` tests +796
(`0xfcf4c`) and enters dispatchLassoCrop (`0xfcf5c`). That route configures/enables
this child (`0xfd06c`, `0xfd084`), then calls ViewGroup::DispatchTouch (`0xfd0c0`).
In View, enabled/hit-tested Down child dispatch (`0x72f18–0x72fa4`) uses slot 136;
Lasso's inherited slot resolves to ViewGroup::DispatchTouch. Its fallback reaches
View::DispatchTouch `0x7021c`, whose enabled branch calls slot 608 (`0x70280`),
resolved by ObjectControl relocation `0x15b0e0` to Lasso onTouch `0xd76dc`.
Non-Down dispatch with active state (`0x72c10–0x72c34`) reaches
ViewGroup::dispatchTouchToChildren `0x73ee0`: its TouchType-1 active-child branch calls
slot 136 (`0x74080`), and admitted fallback reaches View::DispatchTouch
(`0x741a8–0x741b8`, `0x74258–0x74260`). This closes the conditional completion route.
ObjectControl onTouch's accepted action-1 curve/intersection/size branch calls
slot 952 (`0xd7914`), relocation `0x15b238` → cropSelectionArea `0xd7cf4`.
Its slot 976 call (`0xd7db0`), relocation `0x15b250`, reaches saveImageBuffer `0xd92dc`.

saveImageBuffer image mutations require a nonnull type-3 object (`0xd9300–0xd9338`). It attempts
original capture **only if GetOriginalImagePath returns a null pointer** (`0xd9348`);
a nonnull empty string also skips capture. In that branch, current main path must
be nonnull and !IsEmpty (`0xd948c–0xd949c`), then SetOriginalImage(current path)
is called (`0xd94c8`) with its bool ignored. It constructs the edit-buffer Bitmap
(`0xd9394`), handles current flip flags (`0xd93b4–0xd93fc`), calls
ComponentImage::SetImage(Bitmap*) (`0xd9404`) and SetTransparent(true) (`0xd9410`).
Bitmap construction and image setter results are ignored. This transparency is
the separate runtime image bool, not fractional fill alpha. Its final bool is
**initial object pointer nonnull** (`0xd9460–0xd9464`), even for a wrong-type object;
it is not a source-replacement success certificate.

## Generated SPI source, failure path and ordinary save are separate

Model ComponentImage bitmap SetImage `0x3a1968` reaches overload `0x3a1974` and
AddImage(Bitmap, Rect, width 0, codec 7) (`0x3a19cc–0x3a19d4`). From temporary
index +164 it obtains media ID/path (`0x3a19ec`, `0x3a1a00`), then calls the current
String setter (`0x3a1a14`); temporary entry removal is separate (`0x3a1a30–0x3a1a3c`).
ImageCommon::AddImage `0x2b5868` generates a cache-directory/UUID path with `.spi`
(codec default branch `0x2b5a74`, string `0x1346c2`), calls BitmapFactory::SaveBitmap
with quality 100 (`0x2b5a90`), and checks its bool (`0x2b5a94`) before the path-entry
helper (`0x2b5aac`). This file-backed resource can become current main source.
[SPI extension dispatch and framing](spi-media-findings.md) are separate established findings.

False SaveBitmap returns -1 through `0x2b5b00–0x2b5b30` → `0x2b5a2c`. A negative
intermediate index (`0x3a19e0`), or null Bitmap, reaches String SetImage(null)
(`0x3a1a54–0x3a1a70`), which can attempt empty live-fill installation subject to
its own admission. It does not simply return false while preserving the old main.
The lasso caller ignores its final bool; capture and installation remain separate attempts without an atomicity or rollback guarantee here.

The ordinary shape writer selects current Fill GetBinary (`0x3a9070`, relocation
`0x494af8`), which in normal mode writes current ImageCommon media ID (`0x3b90fc`,
`0x3b91c4`). Image own data separately writes original index +180/GetMediaId when
admitted (`0x3a4770–0x3a4794`); a negative index skips it. Current references do not
certify bytes, final ZIP inclusion or original retention: reuse the
[resource/save gates](coedit-resource-findings.md#pending-callbacks-and-ordinary-saving-use-different-gates).

Rust `crates/sdocx/src/image.rs:226` retains original_media_id separately from main;
`storage.rs:216–218` borrows the caller's original page bytes. `container.rs:526–534`
admits JPG/JPEG/PNG/WebP assets, excluding SPI. If the manifest names an existing
`.spi` entry for the main binding, `media.rs:35–42` reports unsupported media rather
than carrying those bytes as a typed asset. The generated cache suffix alone does
not prove every final archive filename/bytes. Original archive preservation is
[separate from parsed source state](vector-retention-findings.md#original-page-bytes-are-external-to-the-parsed-model);
a lasso-generated bitmap does not establish a generic persisted vector mask.
