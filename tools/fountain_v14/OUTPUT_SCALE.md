# V14 vector output-scale source sampling

The explicit output-scale option now resamples centers through the existing V14
sampler before serializing vector paint. It does not multiply page coordinates;
the caller still supplies the matching destination scale. Default rendering is
unchanged. Failed sampler admission retains the canonical prepared stroke.

`output-scale-comparison.json` records an isolated comparison of baseline
`28f20dcf` and that baseline plus the three Rust changes (ink.rs helper,
render.rs invocation and option documentation, fountain_v14_raster.rs test).
The native receipt and saved source were already tracked. At 2x all three tested
readers improve; 1x outputs remain byte-identical. Nonzero error remains.
This single saved-source host control does not establish general page, device,
arbitrary-scale, or native-PDF parity. Compiler and transitive dependency closure
are not pinned by this measurement.

To reproduce, create clean archives of the baseline and delivered commit in
separate temporary directories. Copy the delivered
`crates/sdocx/examples/fountain_v14_scale_probe.rs` into the baseline archive.
Run in each archive, sequentially, substituting `baseline` or `candidate`:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 \
cargo run -p sdocx --features pdf,serde --example fountain_v14_scale_probe -- \
  /tmp/fountain-scale-exports/candidate
```

From the delivered checkout, with resvg on PATH and Python containing Pillow,
PyMuPDF and pypdfium2 at the receipt's versions:

```sh
python3 tools/fountain_v14/output_scale_compare.py \
  /tmp/fountain-scale-exports /tmp/fountain-scale-replay.json /tmp/candidate-archive
```

Compare replay results and artifact hashes with the retained receipt. The
comparator also rejects PDFs containing raster image objects. Generated SVG/PDF
and PNG files are reproducible intermediates and need not be retained. The
fixture-backed Rust integration test independently verifies every serialized
center at 1x and 2x; it fails on the baseline's 2x output.
