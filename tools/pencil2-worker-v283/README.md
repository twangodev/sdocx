# Pencil2 producer to native AGLL queue

After the v219 preparation, run `python3 tools/pencil2-worker-v283/probe.py` and `PENCIL_SKIP_DRAW=1 python3 tools/pencil2-worker-v283/probe.py`. This extends v282 by constructing the AGLL owner before the synthetic worker and returning its native message-queue interface (owner + 8) to the producer. The producer reaches native `enQueueMsg`; there is no host enqueue of the captured Pencil2 message. Five native submissions have vtables `0x1077ba8`, `0x10774f8`, `0x10774c8` (Pencil2), `0x1077528`, and `0x30c3070` in that order. Native `RunTasks` invokes each Run and returns from each slot 8 in FIFO order, and a second drain is empty. The original manager canvas, attachment, drawable, and matrix are asserted. Draw and skip reproduce the v282 322-byte and zero-byte changes and frame hashes, respectively.

This is single-threaded ARM64 emulation with host Mesa. The constructor's initial task Run is skipped before producer submission; its slot 8 is run. Locks and logging are stubbed and compare-and-swap and pointer exchange use host adapters. The queue is passed through a host context-query intervention, the manager canvas factory is intercepted to return the host-created native canvas, and the framebuffer is bound for readback. A returned slot 8 does not establish destruction or resource lifetime. No concurrent scheduling, normal GL-thread startup, saved-page traversal or device behavior is established. Mesa and transitive dependencies are not fully pinned. The image is the previous v274 comparison fixture; this adds no new Rust, SVG or PDF comparison. The isolated wrapper propagates inner result details when no frame is produced.

The probe asserts the initial task's slot-8 entry and the following dispatcher instruction, and checks the complete event sequence again after the second drain. The phase hooks are instruction-site observations: `0x904faf0` is the per-task dispatch, `0x904fb00` follows the indirect Run call, and `0x904fb10` follows the indirect slot-8 call. The mapped base is `0x9000000`. The renderer SHA-256 is checked in the probe; inspect the underlying instructions with:

```sh
LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \
  scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump \
  -d -C --start-address=0x4fac8 --stop-address=0x4fb28 \
  scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so
```
