# V14 vector output-scale source sampling

The explicit output-scale option now resamples centers through the existing V14
sampler before serializing vector paint. It does not multiply page coordinates;
the caller still supplies the matching destination scale. Default rendering is
unchanged. Failed sampler admission retains the canonical prepared stroke.

`output-scale-comparison.json` records a historical isolated comparison of baseline
`28f20dcf` and that baseline plus the three Rust changes (ink.rs helper,
render.rs invocation and option documentation, fountain_v14_raster.rs test).
The native receipt and saved source were already tracked. At 2x all three tested
readers improve; 1x outputs remain byte-identical. Nonzero error remains.
This single saved-source host control does not establish general page, device,
arbitrary-scale, or native-PDF parity. Compiler and transitive dependency closure
are not pinned by this measurement.

The export comparator has been retired. Its historical checks also rejected PDFs containing raster image objects. The fixture-backed Rust integration test independently verifies every serialized center at 1× and 2×; it fails on the baseline's 2× output. This maintained source-sampling check does not reproduce the external reader measurements.
