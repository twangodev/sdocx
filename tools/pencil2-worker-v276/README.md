# Native manager release control

Run after v219 preparation:

```sh
python3 tools/pencil2-worker-v276/probe.py
PENCIL_SKIP_DRAW=1 python3 tools/pencil2-worker-v276/probe.py
```

This extends the v275 manager control with the real native `ReleaseCanvas` body and scoped host adapters for `glDeleteBuffers`, `glDeleteFramebuffers`, `glDeleteTextures`, and `glDeleteSamplers`. Each adapter copies the native array of names into host memory and calls Mesa; `glIs*` checks show each name denoted an object before deletion and no longer did afterward; they do not prove underlying storage was destroyed while references may remain. Both controls observed, in order, buffer 3, framebuffer 2, texture 5, and sampler 5; cleanup returned with no GL error and the manager map reset to empty. The IDs are specific to this isolated host run. The canvas release was called once with the manager's canvas. The native call's manager map membership and the GL deletion sequence establish this control's ownership path; they do not establish all allocations were freed.

The inherited Draw and skip-Draw checks read the framebuffer before cleanup and distinguish the changed frame from the unchanged control. Cleanup destroys the bound framebuffer, so readback after cleanup is not the same test. The message uses a substituted canvas and explicit drawable initialization; normal queue scheduling, saved-page lifecycle, device behavior and resource ownership beyond this synthetic path remain unverified. Transitive runtime dependencies and Mesa are not fully pinned.
