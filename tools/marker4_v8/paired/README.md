# Marker4 V8 paired capture controls

`boundary_capture.py` retains the historical source bytes. Its former
`boundary-receipt.json` is preserved as the JSON string at
`paired_receipts_original.boundary` in
`conformance/marker4-v8-thin-host-native.json`.

Use the maintained entrypoint from any working directory:

```sh
python3 /path/to/sdocx/tools/marker4_v8/paired/verify_boundary.py
```

It verifies the same pinned native runner, fixture and Marker4 library, checks
the same stroke 35 input, then checks sizes 0.99, 1.0, 1.5 and 1.99 against the
retained pixel hashes and alpha counts. Each runner and its captures live in a
temporary directory and are deleted on exit. It prints each verified result;
it does not rewrite receipts or publish captures under `/tmp` fixed names.
Native execution still requires the historical project-local runtime and
transitive dependencies. Host Mesa results do not establish device parity.

The maintained entrypoint was checked statically against the historical source
substitutions at all four sizes. Its native execution has not been validated as
part of the refactor.
