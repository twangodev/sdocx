# Historical fixture validation

The structural model was checked against three independently produced Samsung
Notes documents that have since been retired from the test workflow. These
measurements preserve the evidence for the format map; they are not current
corpus coverage. See [`conformance/README.md`](../../conformance/README.md) for
the maintained corpus and test commands.

The labels used here and in the format map identify the archived inputs by
SHA-256, without depending on the old sample filenames:

| Fixture | SHA-256 |
| --- | --- |
| A | `77e3997a066afa0333d0f5020bb428efffeb783c741bc956ae596292e2d5cda3` |
| B | `38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a` |
| C | `fa2d3ba44023871c6a53436e810772f7b4f45b190dd28e05c172886b8f7e40a0` |

## Stroke and frame audit

| Fixture | Strokes | Points | Base frame | Frame errors |
| --- | ---: | ---: | --- | ---: |
| A | 2,769 | 321,776 | 121 bytes for all | 0 |
| B | 3,228 | 431,933 | 121 bytes for all | 0 |
| C | 1,185 | 170,733 | 121 bytes for all | 0 |
| total | 7,182 | 924,442 |  | 0 |

All audited strokes are compressed and include tilt/orientation channels. For
every stroke:

- the outer declared size ends at its 32-byte hash;
- base-frame size plus stroke-frame size equals outer payload size;
- calculated compressed channel bytes end exactly at the stroke frame's
  `flexible_offset`;
- the next record begins at the declared structural boundary.

## Observed stroke masks

| Fixture | Property masks |
| --- | --- |
| A | `0x25` × 2,732; `0x05` × 37 |
| B | `0x25` × 2,578; `0x05` × 644; `0x65` × 6 |
| C | `0x25` × 1,095; `0x425` × 73; `0x05` × 17 |

| Fixture | Flexible-field masks |
| --- | --- |
| A | `0x258a` × 2,682; `0x258e` × 87 |
| B | `0x248e` × 6; `0x258a` × 2,859; `0x258e` × 363 |
| C | `0x258e` × 1,185 |

## Page and integrity audit

Each fixture has one layer. The page flexible fields end exactly at its layer
offset; each layer object tree ends exactly before its layer hash; and each
file ends with a 32-byte page hash plus the exact 26-byte page signature.

Recomputed results:

- 7,182 object identity hashes: zero mismatches.
- 3 layer hashes: zero mismatches.
- 3 page hashes: zero mismatches.
- Each `pageIdInfo.dat` page hash equals the corresponding page trailer.
- Each `pageIdInfo.dat` note hash equals the `note.note` trailer.
- Each `note.note` trailer equals SHA-256 of the preceding raw note bytes.

## Visual corruption audit

The legacy shifted-stroke fallback selected a corrupt 37-point interpretation
for every affected short stroke:

| Fixture | Corrupt selections | Correct normal alternative |
| --- | ---: | ---: |
| A | 74 | 74 |
| B | 68 | 68 |
| C | 49 | 49 |

This matches the source-level finding that `0x25` is a property mask being read
at the wrong offset as the integer 37.

## Historical Rust regression validation

The structural stroke decoder reproduced the above stroke/point totals and
complete pressure/timestamp/tilt/orientation arrays for all three fixtures.
Every coordinate was within two document units of its separately stored stroke
bbox (allowing fixed-point and floating-point rounding).

The retired runner failed against the previous decoder: fixture A had 322,406
decoded points instead of 321,776. This established a geometry regression,
not visual fidelity for every pen style or non-stroke object. Current synthetic
stroke regressions run in `structural_strokes.rs` without these documents.

## Media manifest regressions

The image migration extended that historical fixture runner to parse
`media/mediaInfo.dat` and verify every manifest digest against the referenced
asset bytes:

| Fixture | Manifest version | Media entries | Hash mismatches |
| --- | ---: | ---: | ---: |
| A | 5202 | 1 | 0 |
| B | 5400 | 8 | 0 |
| C | 5400 | 12 | 0 |

All 21 manifest records had no unknown trailing bytes. Assets included PNG,
PDF and proprietary SPI. This established manifest parsing/hash evidence;
these fixtures did not establish standalone-image rendering fidelity.
