# TapePen package readiness

`check_package.py` checks an explicitly supplied historical source package. It
does not run or replace `preflight_candidate.py`, inspect native code, import
the supplied Python files, or establish capture/runtime parity. The original
preflight includes separate ABI, callback and behavioral checks that this tool
does not attempt.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tools/tapepen-public-drawing/check_package.py \
  --package-dir /absolute/path/to/historical/package \
  --workspace-root /absolute/path/to/its/workspace
PYTHONDONTWRITEBYTECODE=1 python3 tools/tapepen-public-drawing/test_check_package.py
```

Both roots are required; no untracked research package is selected implicitly.
The package must supply the V7 `preflight.json`, V25 `candidate.json`, runner,
canvas helper and original preflight script. The checker emits a new V1 JSON
result on stdout and exits nonzero when package admission is blocked.

Results distinguish invalid documents/paths, missing files, changed hashes,
unreadable files and historical locations requiring reconciliation. The five
phases check document schemas, local source identities, the candidate/manifest
hash join, recorded invocation locations, and declared workspace input hashes.
Pins resolve within the workspace; package sources resolve within the package.
Paths escaping those roots through `..`, absolute dependency paths or symlinks
are rejected before reading the referenced file.

The recorded command and preflight path declarations remain in the result.
`relocation_required` means a declaration points somewhere other than the
caller-supplied adjacent package file; it is not evidence of changed bytes and
does not authorize rewriting the historical command. In the retained local
package, source/manifest identities match, but recorded locations name earlier
temporary investigation directories. Preserve that distinction when deciding
how to reproduce an old run.

Even a fully admitted package reports `behavioral_preflight: not_run`. This
inventory covers declared inputs only; host libraries, toolchain behavior and
the full transitive runtime closure remain outside its claim. Old capture
receipts and original source hashes continue to describe their original
producers.
