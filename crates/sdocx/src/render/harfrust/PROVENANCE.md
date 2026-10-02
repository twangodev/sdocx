# HarfRust source provenance

This private SDK module contains the published `harfrust` 0.13.3 package's
source, MIT license, and upstream README. It is compiled only with the SDK's
`render` feature. Cargo packages this source with `sdocx`; there is no path
dependency or build-time dependency-source rewriting.

| Artifact | Pinned value |
| --- | --- |
| Published package | [`harfrust` 0.13.3](https://crates.io/crates/harfrust/0.13.3) |
| Package archive | `harfrust-0.13.3.crate` |
| Archive SHA-256 | `948d0741125ba89cd3e1c23e5642415b6ade7e1d29d67ba25fb925b533e989d6` |
| Upstream revision | [`fbff7e563b4e512686c9e627d91a147ba4ee2402`](https://github.com/harfbuzz/harfrust/tree/fbff7e563b4e512686c9e627d91a147ba4ee2402/harfrust) |
| Upstream shaping baseline | HarfBuzz 14.3.1 |

The maintained rounding delta is in `hb/face.rs`, with its export in `mod.rs`.
`ShapeOptions::integer_scaling_rounding` selects `IntegerScalingRounding::Nearest`
or `Floor`. The default retains upstream's signed 16.16 multiply followed by a
32768 bias and a right shift. The explicit `Floor` policy removes that bias
before the shift for integer font-coordinate scaling. Multiplier construction,
font callbacks, fractional variation deltas, extent rounding, parsing, Unicode
processing, and shaping lookups retain upstream behavior. An unspecified shape
scale still returns font-unit geometry.

`hb/buffer.rs` also exposes optional per-buffer glyph-storage and tracked-operation
limits, plus the existing shaping-success state. The default has no additional
limits. An explicit limit clamps the upstream limit when shaping starts;
clearing the buffer restores the defaults. SDK shaping rejects failed output.
The operation limit measures HarfRust's existing tracked operations, not every
CPU operation. Custom SDK regressions include a real GSUB expansion beyond the
storage cap and contextual lookup exhaustion; they are separate from the
disabled upstream tests.

The floor policy matches the executed Samsung horizontal `ValueFormat4`
`x_advance` path in
[`table-text-shaping-gpos.json`](../../../../../conformance/table-text-shaping-gpos.json).
That capture establishes the signed floor operation and exact source-font
operands for four supplied Roboto cases. It does not establish every GPOS
format, variable-font positioning, font fallback, or complete document layout.

Embedding applies these mechanical adaptations to upstream source:

- `src/lib.rs` becomes `mod.rs`; `crate::` paths become
  `crate::render::harfrust::` paths.
- Upstream `std` feature conditions become SDK `render` conditions; this
  private module always compiles with the standard library.
- Upstream test conditions and standalone tests are disabled with `any()`
  conditions. The published package does not contain the upstream test fonts.
  Hash-pinned SDK capture regressions exercise the maintained native contracts.
- Two borrowed `NonContextual` patterns in `hb/aat/layout_morx_table.rs` omit
  redundant `ref` modifiers for Rust edition 2024.
- Formatting and trailing whitespace follow the SDK's Rust edition and checks. Third-party lint and unused API
  allowances are confined to the private module declaration.

The upstream README and license are retained verbatim. README references to
upstream-only examples and tests describe the original project.
