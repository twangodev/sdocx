# Conformance

Rust fixture tests are part of the normal workspace checks:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The CI workflow uses `cargo nextest run --profile ci --all-features` for the
workspace test job, including the Rust visual-tooling tests.

Native JSON fixtures retain the inputs and expected values asserted by Rust
tests. Full capture traces and their original digests remain in the historical
revision below; reverse-engineering evidence links point to those snapshots.
Add regression logic in Rust and keep new fixture data limited to its assertions.

## External compatibility corpus

Large `.sdocx` files and Samsung reference PDFs are stored in the
[compatibility dataset](https://huggingface.co/datasets/twangodev/sdocx-compatibility).
Initialize the pinned submodule and fetch its LFS assets when running corpus
checks:

```sh
git submodule update --init hf
git -C hf lfs install --local
git -C hf lfs pull
cargo test -p sdocx --all-features --test conformance -- --ignored
```

[`corpus.json`](corpus.json) locks fixture hashes and page expectations;
[`manifest-format.md`](manifest-format.md) describes its fields and
[`fixture-capture.md`](fixture-capture.md) documents adding a fixture. A
different local checkout can be selected with `SDOCX_CORPUS_DIR`.

## Visual comparison

`sdocx-conformance` compares CLI PNG or PDF output with the locked Samsung export
corpus. Initialize the `hf` submodule and its LFS files when needed, then run:

```sh
cargo build -p sdocx-cli
cargo run -p sdocx-conformance --bin visual -- \
  --pdf-python conformance/.venv/bin/python --output tmp/visual/current
cargo test -p sdocx-conformance
```

`visual` accepts `--corpus-dir` (or `SDOCX_CORPUS_DIR`), `--manifest`,
repeatable `--fixture ID`, `--cli`, `--format png|pdf`, repeatable `--font`,
`--channel-threshold`, `--ink-threshold`, `--tolerance` (0–5), and required
`--output`. Output directories must be new. The corpus is not downloaded or
modified by the runner.

`ink` compares selected stroke regions in existing images:

```sh
cargo run -p sdocx-conformance --bin ink -- \
  geometry.json reference.png candidate.png [--profile Marker4] \
  [--settings '7;'] [--region LEFT TOP RIGHT BOTTOM] [--chroma-threshold N]
```

Metrics, reports, ink selection and their tests are Rust. PDF rasterization
uses a small external PyMuPDF adapter; install `pymupdf==1.28.2` in the Python
environment passed with `--pdf-python` (or `SDOCX_PDF_PYTHON`). Its Python and
PyMuPDF versions are recorded in each report. The corpus is never downloaded
by the runner. Native-capture Python scripts are no longer maintained in this
checkout. Their full sources are
retained in the ignored archive at
`tmp/conformance-cleanup/native-capture-a241793` and can be restored from
revision `a24179384fbe043a8b5f003ee0c7bebeb394d106` with:

```sh
mkdir -p tmp/conformance-cleanup/native-capture-a241793
git archive a24179384fbe043a8b5f003ee0c7bebeb394d106 conformance | \
  tar -x -C tmp/conformance-cleanup/native-capture-a241793
```

The standalone `conformance/native_table.rs` harness remains available for
manual native investigations. Its build and APK-library requirements are
checkout-specific; it is not part of ordinary CI.
