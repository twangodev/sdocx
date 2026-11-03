# Expanded viewport native control

`native.json.gz` is the original synthetic host Mesa capture, compressed with
`gzip -n -9`. Its uncompressed SHA-256 is
`ff2482645a0140356228da84aa93461dc840227af132d5d3b8ba9c2ab387fe0c`.
The receipt pins the original capture script and native libraries. This is not
saved-note or device evidence.

The capture script is `tools/pencil2_v14/expanded_viewport_capture.py`. Its
pinned base source can be restored from the tracked
`tools/pencil2_v14/context-base-capture.py.gz` to the script's `SOURCE` path.
Recapture also requires the historical `tmp/vector-parity-plan` inputs and the
pinned native libraries; the base source lists those inputs. The script writes
an uncompressed receipt under `conformance/`; compress that output with
`gzip -n -9` to update this fixture after a successful recapture.
