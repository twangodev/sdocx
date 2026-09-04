# Compatibility fixture validation

The structural model was checked against three independently produced Samsung
Notes documents. These documents remain outside the repository.

## Stroke and frame audit

| Fixture | Strokes | Points | Base frame | Frame errors |
| --- | ---: | ---: | --- | ---: |
| handwritten | 2,769 | 321,776 | 121 bytes for all | 0 |
| quiz | 3,228 | 431,933 | 121 bytes for all | 0 |
| CS61BL | 1,185 | 170,733 | 121 bytes for all | 0 |
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
| handwritten | `0x25` × 2,732; `0x05` × 37 |
| quiz | `0x25` × 2,578; `0x05` × 644; `0x65` × 6 |
| CS61BL | `0x25` × 1,095; `0x425` × 73; `0x05` × 17 |

| Fixture | Flexible-field masks |
| --- | --- |
| handwritten | `0x258a` × 2,682; `0x258e` × 87 |
| quiz | `0x248e` × 6; `0x258a` × 2,859; `0x258e` × 363 |
| CS61BL | `0x258e` × 1,185 |

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
| handwritten | 74 | 74 |
| quiz | 68 | 68 |
| CS61BL | 49 | 49 |

This matches the source-level finding that `0x25` is a property mask being read
at the wrong offset as the integer 37.

## Rust regression validation

The structural stroke decoder now reproduces the above stroke/point totals and
complete pressure/timestamp/tilt/orientation arrays for all three fixtures.
Every coordinate is within two document units of its separately stored stroke
bbox (allowing fixed-point and floating-point rounding).

The repeatable runner and SHA-256 lock are described in
[`../../conformance/README.md`](../../conformance/README.md). The same runner
fails against the previous decoder: handwritten has 322,406 decoded points
instead of 321,776. This is a geometry regression check, not a visual-fidelity
claim for every pen style or non-stroke object.
