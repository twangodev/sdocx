# Pencil2 render-message execution boundary

Run `python3 tools/pencil2-worker-v246/probe.py`. This modifies the existing synthetic synchronous dispatcher in a temporary script. It replaces the invalid bitmap virtual slot 168 call target with a return-self stub, then deliberately stops the native call before a null dereference. This is an intervention, not an established bitmap contract.

The dispatched `PenGLRenderMsg::Run` enters `Pencil2StrokeDrawableRTV4::Update(false)`. The probe asserts the Run message receiver equals the receiver entering Update with `false`. At Pencil2 RVA `0x99640`, Update loads the graphics pointer from drawable offset 200. The probe stops immediately before the dereference at `0x99648` and asserts both that field and the loaded register are zero; it does not execute the invalid read. At Update entry, the probe also asserts the return address is common RVA `0x467f8`. Before the intervention, `drawing_queue_exception_diagnostic.py` detects the message's bitmap slot 168 target `0x203252e`, which is not aligned as an ARM64 instruction address.

The bridge in v244 initializes the drawable explicitly after deferring this message. This probe has not combined that initialization with execution of the queued message. It observes no Draw matrix or paint, and says nothing about normal scheduling, saved pages, or device behavior. The parent script is pinned; its native dependencies and host runtime retain their existing limits.

Replay requires the existing local investigation workspace; these two files alone
are not runnable in a clean checkout. Required inputs include
`tools/pencil2-public-replay/drawing_queue_exception_diagnostic.py` (SHA-256
`7866a4c0bd27c1471409bdf02e271b49c9650a830d5e9539d67f9ced9e15c359`),
`tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py`
(SHA-256 `996ef6db3a376619e9e5d1d05b7d5e1a940726f3feb2ef6895275a705576ee3e`),
and that capture script's pinned adapters, metadata, native ARM64 libraries,
Unicorn, binutils, and host runtime. The latter are listed in its `PINS` mapping;
they are not bundled here. The generated script and capture output directory
are temporary and removed when the subprocess completes, including on failure.

`receipt.json` retains the compact output of a successful local replay, including
the observed PC, LR, receiver addresses and zero graphics values. Re-running the
command emits this JSON to stdout; pointer addresses are local harness values.
