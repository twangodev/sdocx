# Translated Crayon2 sampler intervention

The existing translated record 139 shader interventions encode both background
varying coordinates and independently output the background sample. Across all
nine covered pixels, the decoded coordinates, using binary32 normalized repeat
and the captured 150×150 texture, select the observed background value. This
includes the original boundary mismatch at GL pixel (52, 57). The coordinates'
high bytes are inferred from their expected range; agreeing texture values do not
uniquely identify texels. These recompiled shaders are interventions and do not
prove the unmodified shader's varying or texture-unit implementation. The result
narrows this control's residual toward the interpolation candidate, without
justifying a renderer correction or device parity claim.

`receipt.json` retains the nine packed diagnostic pixels and computed texels.
Verify with `.venv/bin/python tools/crayon2-sampler-intervention/verify.py`.
To compare fresh captures too, follow the environment and capture instructions
in `../crayon2-transform-control/README.md` for record 139 modes `xuv`, `yuv`,
and `background`, naming the JSON files by mode in one directory. Pass that
directory as the verifier's argument; it checks complete frame hashes against
the existing intervention receipt as well as every retained pixel. The native
texture remains a local hash-checked prerequisite.

Fresh translated interventions on records 20, 100 and 200 add eleven covered
pixels, all of which also select the independently sampled background value.
Their varying-frame hashes match the earlier translated UV receipt. Their packed
pixels, inferred high bytes, texture indices, and all three frame hashes are in
`additional-receipt.json`; the verifier checks these too. For fresh comparison,
capture the same three modes for each record in the same directory as above,
with filenames `xuv20.json`, `yuv20.json`, `background20.json`, and so on.

Fresh capture verification checks each mode's source input, pinned dependency
closure, shader source hashes, texture allocations, matrix and texture
uniforms, blend state, draw call, selected native vector record, GL errors and
complete framebuffer hash against the existing intervention evidence. The three
additional background frame hashes are capture receipts, while their shader and
state checks are anchored to the earlier intervention receipt. For each packed
varying, the verifier requires a finite decoded value within `1e-5` of the
geometric expected coordinate, and exactly one such candidate among high bytes
`0x3c` through `0x40`. This is an explicit inference range and tolerance, not
an independently observed high byte.
