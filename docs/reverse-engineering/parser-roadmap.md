# APK-aligned parser roadmap

## Target data flow

```text
SDOCX bytes
  -> appended end tag and ZIP directory
  -> note.note + pageIdInfo.dat
  -> pages in manifest order
  -> page header and flexible fields
  -> layer records
  -> recursive outer object records
  -> generic typed frames
  -> object-specific decoders
  -> stable document model
  -> SVG/PNG renderer
```

## Implementation sequence

The structural stroke milestone is implemented: `StoredPage` drives traversal,
page/layer masks are length-prefixed, and a bounded typed-frame reader handles
compressed/uncompressed stroke channels plus color and pen size. The legacy
stroke walker and shifted fallback are removed. Public header field names are
retained for compatibility. The numbered sequence below remains the overall
roadmap; full common-object metadata, remaining styles, end tags, hashes and
non-stroke semantics are still incomplete.

1. Correct page and note header names: flexible offsets and variable-length
   property/field masks.
2. Make `StoredPage` traversal authoritative for layers and recursive objects.
3. Add a generic typed-frame reader using `frame_size`, relative
   `flexible_offset`, and variable-length masks.
4. Add the type-0 base-frame decoder and expose UUID, modified time, bbox and
   common flexible fields.
5. Add the type-1 stroke decoder for compressed and uncompressed channels.
6. Delete the magic-offset stroke walker and shifted fallback.
7. Decode stroke style fields in mask order; retain unknown fields safely.
8. Cursor-parse the variable-length inner and appended end tags.
9. Add optional validation for note, manifest, object, layer and page hashes.
10. Move text/image/shape/table/code-block parsing onto the same frame model.

## Compatibility rules

- Manifest page order overrides ZIP order.
- The post-EOCD end tag is authoritative and can differ from `end_tag.bin`.
- Modern attachments use `note.note` plus `media/`; do not generate the generic
  `attach/attachInfo.dat` form.
- Do not require a particular ZIP compression method.
- Preserve unknown object IDs and unexpected trailing manifest bytes.
- Apply configured bounds before allocating page, object, text or point arrays.
- Do not equate identity hashes with payload authentication: Samsung's object
  hash covers UUID plus modification time, not raw object bytes.

## Remaining research

- Multiple layers and recursive child/container fixtures.
- Full non-stroke frame layouts, especially math and newer native table/code
  objects.
- Page custom-object internals and a few legacy common fields.
- Proprietary `.spi` payload semantics.
- Byte-for-byte encrypted-file validation with a protected fixture.
