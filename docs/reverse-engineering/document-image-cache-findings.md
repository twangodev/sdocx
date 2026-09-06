# Document image-cache persistence

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so`, `libSPenGraphics.so` and `libSPenBase.so` from the
[identified APK](README.md#sources-and-validation). Addresses are in
Composer unless prefixed with Graphics or Base.

The [save-preparation trace](composer-close-findings.md#ready-for-save-reaches-the-document-image-cache)
reaches `DocumentImageCache::SaveCache`, `0x53c61c`. This investigation
connects that cache to `.spi` bitmap files and page canvas-cache metadata.
It does not establish the final archive packaging or ordinary stroke
serialization.

## SaveCache submits bitmap work and waits for its completion

`SaveCache` reads its writing-document pointer at member 120 through
`0x53c668`. A null pointer skips the cache work. Otherwise it calls
`SPBitmapLoader::CancelAllLoadRequest` at `0x53c674`, through relocation
`0x5ab7d8`, before acquiring its critical section.

The function examines per-layer cache state, may request cache drawing
through `0x53c8ac`, and calls `saveCache(int, int, ISPBitmap*)`,
`0x53ca20`, at `0x53c7b8` for eligible entries. This does not mean that
every layer necessarily produces an image on each save request.

After releasing the critical section at `0x53c818`, it obtains an argument
through its member-112 object's slot 24 and calls
`SPBitmapLoader::WaitForAllSaveRequestsToEnd(IGLMsgQueue*)` at `0x53c82c`.
Relocation `0x5ab7e0` identifies this operation. It then calls
`saveFilePathToDocument`, `0x53b038`, at `0x53c834`.

Graphics `WaitForAllSaveRequestsToEnd`, `0x877dc`, obtains the
`BitmapLoaderImpl` singleton, calls `WaitUntilMsgQueueRequired` at
`0x877f4`, dispatches the supplied object's slot 24 at `0x87804`, then
enters `BitmapLoaderImpl::WaitForAllSaveRequestsToEnd`, `0x8563c`.
The latter holds its critical section while checking two request lists
and its active-save byte:

| State | Graphics evidence |
| --- | --- |
| Member-56 request list is nonempty | `0x8569c` through `0x856a8` |
| Member-48 request list is nonempty | `0x856ac` through `0x856b8` |
| Member-137 active-save byte is set | `0x856bc` through `0x856c0` |
| Any condition remains true | Conditional-variable wait at `0x85700`, then repeat the checks |

This is a concrete wait for the bitmap loader's save work. It does not
establish delivery of the separate
[prediction completion Handlers](predictor-queue-findings.md).

On a normal return, `SaveCache` reports whether its initially loaded
document pointer was nonnull, at `0x53c83c` through `0x53c840`. That
Boolean does not aggregate individual image-write results.

## Cache keys determine internal SPI filenames

`saveCache` combines its integer page and layer arguments at `0x53ca80`
through `0x53ca88`:

```text
cache_key = page_id + 1_000_000 * layer_id
```

The arithmetic uses 32-bit registers. This is a cache-key construction,
not proof of valid page-ID ranges or a media-manifest bind-ID rule.

The path helper, `0x53d984`, requests the document's internal directory
through slot 536 at `0x53d9c8`, then appends the format string
`/page_%07d.spi` from `0x1dc26d`. Formatting and appending occur at
`0x53d9e0` and `0x53d9ec`. Decimal width 7 is minimum padding; it does
not truncate larger keys. For example, page 7 and layer 2 produce
`page_2000007.spi`.

Both main-editor writing adapters resolve slot 536 to
`WNote::GetInternalDirectory(String*)`:

| Adapter | Slot entry | Target |
| --- | --- | --- |
| `NoteWritingWNote` | `0x576030` | `0x4faac0` |
| `NoteWritingContinuousWNote` | `0x574f58` | `0x4f02f8` |

Each target loads its WNote member 88 and branches through relocation
`0x5a53f0`. The final archive's media prefix and manifest binding remain
separate from this internal filename.

## The save request contains pixels and a codec parameter

`saveCache` queries document objects for the selected page and layer,
requires a successful result with a nonempty `ObjectList`, and checks
that its bitmap argument is nonnull at `0x53cb30` through `0x53cb58`.
It submits that bitmap at `0x53cb74` through relocation `0x5ab7f8`:

```text
SPBitmapLoader::RequestSave(cache_key, path, bitmap, null, 100)
```

Graphics `SPBitmapLoader::RequestSave`, `0x86f20`, forwards the request
to `BitmapLoaderImpl::RequestSave` at `0x86f7c`. The implementation,
`0x8501c`, allocates a request and a width-times-height-times-four byte
buffer. It stores the supplied ID, path, pixel buffer, callback data and
integer parameter; the latter is retained at request offset 88 through
`0x85110`. Bitmap readback is requested through slot 48 at `0x851f0`
or slot 40 at `0x85224`, depending on the worker branch.

Graphics `BitmapLoaderImpl::SaveBitmap`, `0x84388`, wraps the request's
pixel buffer in a `Bitmap` and calls `BitmapFactory::SaveBitmap` at
`0x8440c`, through relocation `0xd7718`. It passes the path and saved
integer parameter. Base `BitmapFactory::SaveBitmap`, `0xa94fc`, selects
`write_maetel_argb` for the `.spi` extension. The
[SPI media trace](spi-media-findings.md) follows that codec wrapper.

## Completed paths become page canvas-cache metadata

`saveFilePathToDocument`, `0x53b038`, processes its pending path map when
member 272 equals 2. At `0x53b0dc` through `0x53b0f8`, it splits each
combined key using signed division and remainder by 1,000,000 and calls
`setCacheImageToDocument`, `0x53da18`.

That helper builds cache metadata and calls writing-document slot 392 at
`0x53db78`. The two adapters store it differently:

| Adapter | Slot entry and target | Document operation |
| --- | --- | --- |
| `NoteWritingWNote` | `0x575fa0` → `0x4fa650` | Finds the WPage by page ID at `0x4fa67c`; calls `WPage::SetCanvasCacheData(layer_id, data)` at `0x4fa6d8` |
| `NoteWritingContinuousWNote` | `0x574ec8` → `0x4eff04` | Gets WNote page 0 at `0x4eff84`; recombines page and layer IDs at `0x4eff90`; calls `WPage::SetCanvasCacheData(cache_key, data)` at `0x4eff9c` |

Relocations `0x5ab5e8` and `0x5ab5e0` identify the
`WPageData::CanvasCacheData` constructor and page setter. These calls
establish canvas-cache persistence, not insertion of stroke point data.
The metadata's serialized record layout and the complete callback/state
machine remain unresolved.

## Validation and SDK implications

The APK digest and all three ELF streams were checked against the archive.
Cited instructions, slot bindings, extension strings and imported methods
were checked against their binary bytes. Disposable arithmetic checks
covered cache-key construction, signed splitting and filename padding.

The SDK can keep page-cache discovery distinct from editable object
decoding. Native code provides a concrete `.spi` producer and canvas-cache
association, but this trace alone does not justify replacing a page's
objects with its cache image. No device execution, new SDOCX fixture or SDK
implementation was used.
