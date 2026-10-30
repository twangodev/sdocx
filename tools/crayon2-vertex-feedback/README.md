# Crayon2 vertex transform feedback

For retained record 139, the original Crayon2 vertex shader is linked with
`gl_Position` and `vBgCanvasUV` as transform feedback varyings. After the
normal draw, a second draw runs with rasterizer discard and captures six
vertices emitted from the four-vertex triangle strip, in order 0, 1, 2, 2,
1, 3. The shader and its uniforms are unchanged; adding transform feedback
can affect compilation, so these are intervention results. Both identity and
(13, 9) translated view captures returned GL error zero. All six captured
position and UV records exactly match the current binary32 vertex model. This supports the vertex calculation for this stamp under this intervention;
it does not prove the unmodified program produced identical vertex outputs and
therefore does not conclusively locate the translated discrepancy; it does not prove the original fragment varying, interpolation setup,
texture sampling, arbitrary stamps, or device behavior. The feedback draw
has no framebuffer output.

The compact receipt contains each 144-byte interleaved float stream and its
SHA-256. Verify the calculation with `.venv/bin/python
 tools/crayon2-vertex-feedback/verify.py`. The diagnostic uses the same local
native dependencies and environment as the [document holdout](../crayon2-document-holdout/README.md).
With that environment set, reproduce from the repository root:

```sh
.venv/bin/python tools/crayon2-vertex-feedback/capture.py > /tmp/crayon-tf-translated.json 2> /tmp/crayon-tf-translated.log
CRAYON_IDENTITY=1 .venv/bin/python tools/crayon2-vertex-feedback/capture.py > /tmp/crayon-tf-identity.json 2> /tmp/crayon-tf-identity.log
```

Each log's `TF_CAPTURE` hex should equal the corresponding receipt and report
`error 0`. The JSON files are disposable native capture intermediates; they
are not used as renderer input.

The capture asserts successful program linkage, the original strip draw arguments,
zero preexisting transform-feedback generic and indexed buffer bindings, and
disabled rasterizer discard. It checks those bindings and discard state are
restored and GL errors are zero after capture. The receipt pins the transitive
capture scripts, recorded native dependency and host GL hashes, shader sources,
uniforms, draw state and host GL identity. The recorded GL library hashes do
not cover the complete Mesa driver and LLVM dependency closure. Compare both fresh outputs and logs
against that context with:

```sh
.venv/bin/python tools/crayon2-vertex-feedback/verify.py /tmp/crayon-tf-translated.json /tmp/crayon-tf-translated.log /tmp/crayon-tf-identity.json /tmp/crayon-tf-identity.log
```
