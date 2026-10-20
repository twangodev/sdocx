# Conformance

Rust fixture tests are part of the normal workspace checks:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The CI workflow uses `cargo nextest run --profile ci --all-features` for the
workspace test job and runs the Python visual-tooling tests separately.

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

`visual.py` compares CLI PNG or PDF output with the locked Samsung export
corpus. Initialize the `hf` submodule and its LFS files when needed, then run:

```sh
cargo build -p sdocx-cli
uv run --project conformance --locked python conformance/visual.py \
  --output tmp/visual/current
uv run --project conformance --locked python -m unittest discover \
  -s conformance -p 'test_*.py'
```

`visual.py` accepts `--corpus-dir` (or `SDOCX_CORPUS_DIR`), `--manifest`,
repeatable `--fixture ID`, `--cli`, `--format png|pdf`, repeatable `--font`,
`--channel-threshold`, `--ink-threshold`, `--tolerance` (0–5), and required
`--output`. Output directories must be new. The corpus is not downloaded or
modified by the runner.

`ink_visual.py` compares selected stroke regions in existing images:

```sh
uv run --project conformance --locked python conformance/ink_visual.py \
  geometry.json reference.png candidate.png [--profile Marker4] \
  [--settings '7;'] [--region LEFT TOP RIGHT BOTTOM] [--chroma-threshold N]
```

The maintained Python suite covers visual-tooling tests; CI discovers
`test_*.py`. The native-capture Python scripts are no longer maintained in this
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
