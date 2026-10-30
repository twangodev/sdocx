# Pencil2 synthetic worker and explicit renderer handoff

Run from the repository root:

```sh
python3 tools/pencil2-worker-v242/probe.py
```

The verifier compares a fresh run against `receipt.json`. Use `--write-receipt`
only to deliberately refresh that receipt. Parent adapters are hash pinned;
the retained v219 native harness and its local dependencies are required.

One synthetic request passes through native submission and one EventLoop
iteration, including queue cleanup and empty sentinel checks. The adapter
then explicitly calls renderer Init, Update and Draw through host Mesa.
It asserts the ordered five-event dispatch, 84 AddPoint calls and matching
Matrix3 setters, renderer identity, successful update/draw, zero GL error,
and the retained pixel hash (317 nonzero bytes).

The queue is transplanted into native singleton globals. Bitmap, context,
locking and destructor behavior remain synthetic. The draw Matrix4 is supplied
independently. This establishes neither automatic paint nor Matrix3 conversion,
concurrent scheduling, saved-page fidelity or device behavior. Host EGL, GLES,
Mesa and transitive dependencies remain unpinned.
