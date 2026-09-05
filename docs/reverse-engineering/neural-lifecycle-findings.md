# Neural model replacement and runtime ownership

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` and bundled `libSPenTFLite.so` from the
[identified APK](README.md#sources-and-validation).

This extends [interpreter setup](neural-inference-setup-findings.md) and
[worker scheduling](predictor-worker-findings.md) with replacement order,
failure state, and the lifetime of inference pointers copied into tasks.
All addresses below belong to Predictor unless explicitly labeled TFLite.
The pending-task examples require the specified caller ordering; they are
not measured crashes or proof of application-wide concurrency behavior.

## SetModel replaces entries by input type

`TFLiteHolder::SetModel`, `0x381d4`, requests input types 0 and 1
and calls `AddModel` at `0x3820c` only when `GetModel` returns non-null.
It does not clear the entire map or remove an entry for a null result.
The [bundled accessor](neural-model-findings.md#input-type-controls-access-and-lazy-decoding)
returns a record only for type 1, so that is the entry these ordinary
holder updates replace.

`AddModel`, `0x3822c`, uses the model's integer field at offset 68
as its map key, forming its address at `0x382fc`. The map object begins
at holder offset 184. Lookup/insertion helper `0x38dbc` compares keys
against node offset 32 at `0x38e08`–`0x38e28`. Equal keys return the
existing node through `0x38ea8`; a missing key allocates a 96-byte node
at `0x38e38`/`0x38e3c`.

The entry therefore remains at the same node address when the same input
type receives a different model. That does not preserve the lifetime of
the resources stored inside it.

## Existing resources are cleared before new runtime construction

The temporary entry at stack offset 72 contains the new model-info
pointer, null flat-buffer/interpreter/runner pointers, and an empty
validator map at `0x382f8`–`0x38310`. After lookup, replacement proceeds
in this order:

| Operation | Instruction evidence |
| --- | --- |
| Replace node offset 40 with the new model-info pointer | `0x38338` |
| Reset owned flat-buffer model at node offset 48 to null | `0x3833c`/`0x38340` |
| Reset owned interpreter at node offset 56 to null | `0x38348`/`0x38350` |
| Clear signature-runner pointer at node offset 64 | `0x38354`/`0x38364` |
| Destroy the old validator tree | `0x38358`–`0x38368` |
| Install the empty replacement validator tree | `0x3836c`–`0x38390` |
| Begin constructing the new flat-buffer model | `0x383c0`–`0x383dc` |

The interpreter reset helper at `0x28eb8` first stores its replacement
pointer at `0x28ec8`. If the old pointer was non-null, it calls the
TFLite interpreter destructor at `0x28ed4` and `operator delete`
through `0x28ee4`. The destructor import is Predictor relocation
`0x42478`, reached through PLT `0x3bd50`.

The flat-buffer reset helper at `0x28ef4` likewise replaces its owner
pointer before deleting the old object. It releases that object's owned
allocation through its virtual deleting destructor at `0x28f20`, then
deletes the flat-buffer object through `0x28f30`.

Validator cleanup helper `0x38b2c` recursively visits both child nodes,
invokes each non-null validator's deleting destructor at `0x38b6c`,
and deletes the map node through `0x38b7c`. These are actual resource
destructions, not just removal of lookup aliases.

## Failed setup does not restore the previous runtime

The earlier clearing happens regardless of whether the replacement can
be constructed. On normal return, the setup branches leave:

| Replacement outcome | New model-info pointer | Stored runtime handles | Time validator for bundled `xysotp` |
| --- | --- | --- | --- |
| Flat-buffer construction returns null | Present | Both null | Absent |
| Interpreter construction leaves a null interpreter | Present | Both null | Absent |
| Positional tensor allocation fails | Present | Both null | Present |
| Positional tensor allocation succeeds | Present | Interpreter only | Present |
| Signature tensor allocation succeeds | Present | Interpreter and runner | Present |

The graph failure at `0x383e4` reaches cleanup through `0x38604`.
The null interpreter branch at `0x38450` reaches cleanup through
`0x38620`. Neither traverses the feature-validator loop.

An absent or unusable signature falls back to positional setup. A
nonzero positional allocation result at `0x386ac` logs and branches
through `0x386c8` to the validator-building path at `0x3871c`.
It skips the successful owner transfers at `0x386f0` and `0x38718`.
The unsuccessful temporary interpreter and flat-buffer objects are later
destroyed at `0x38800`–`0x38844`.

On signature success, the new flat-buffer owner is installed at
`0x388f8`, the runner pointer at `0x38914`, and the interpreter owner
at `0x3893c`. There is no branch restoring the old objects after a
failed replacement. A model-info pointer or installed validator alone
does not establish that inference is available.

These failure outcomes describe native branch behavior. They do not
establish that a bundled model fails initialization on a device.

## The signature runner lives inside an interpreter-owned map node

The imported interpreter destructor resolves in bundled TFLite to
`0x25e458`. The runner accessor in that same library is
`Interpreter::GetSignatureRunner`, `0x25f780`.

The accessor searches a map whose root is at interpreter offset 288,
loaded at TFLite `0x25f7c8`. For an existing entry it returns
`node + 56` at TFLite `0x25f840`. New runner construction at
TFLite `0x25f8b8` or `0x25fa2c` creates a temporary value, which
map helper `0x261150` moves into a newly allocated 128-byte node.
The runner's copied storage begins at node offset 56 through TFLite
`0x261240`. The accessor returns that address at TFLite `0x25f960`.

During interpreter destruction, TFLite `0x25e4f0` loads the same root
and calls tree cleanup at `0x25e4f4`. Cleanup helper `0x260404`
recursively visits the children, destroys each node's contents, and
deletes the node through TFLite `0x26043c`. Its delete PLT is
`0x2c1670`, verified by the library's relocation.

Consequently a copied runner pointer cannot keep its runner alive after
the owning interpreter is destroyed. It points inside an allocation that
the interpreter destructor releases. No separate runner ownership is
transferred to the prediction task.

## Pending tasks retain bindings to destroyed resources

`PredictionTask::SetTFParams` copies the model-info pointer, interpreter
pointer, and runner pointer, while retaining the address of the entry's
validator map. Those fields are plain pointer copies at
`0x2befc`–`0x2bf14`; task destruction at `0x2be60` is a no-op
apart from the separate deletion of the task allocation.

For a same-input-type replacement after task binding:

| Task member | Effect of holder replacement |
| --- | --- |
| Model-info pointer, offset 16 | Still points to the earlier model record |
| Interpreter pointer, offset 24 | Still contains the earlier address; that interpreter is destroyed |
| Runner pointer, offset 32 | Still contains the earlier address; its owning map node is destroyed |
| Validator-map reference, offset 40 | Still refers to the same entry's map object, whose contents were replaced |

The previously traced worker permits the following static sequence:

1. Submit a task while idle; it binds model A and remains pending in state 2.
2. Call the worker's `Wait(true)` before the worker begins executing.
3. Replace that holder's input-type entry with model B.
4. Allow the worker to run the already-bound task.

The wait returns for pending state 2 and does not cover replacement with
a held mutex. Neither `SetModel` nor the normal worker execution transition
rebinds the pending task. In this ordering the task keeps stale inference
addresses even if the new model initializes successfully.

`Run` tests its copied handles for null at `0x2c570`–`0x2c578`;
destroying the pointed-to objects does not zero those task members. It
then accesses the signature input tensor at `0x2c624`, or dereferences
the positional interpreter at `0x2c634`–`0x2c638`. These accesses
precede the retained-point/minimum-count comparison at `0x2c704`.
That later count check is therefore not a guard for this lifetime edge.

This establishes a missing local lifetime guarantee for the specified
sequence. Whether outer callers exclude it remains unresolved. It does
not establish an observed use-after-free in Samsung Notes.

## Predictor teardown preserves the distinction between its two holders

`NNPredictor` destruction releases its local holder at
`0x25150`/`0x25158`, then destroys its worker at `0x25168`.
The worker requests stop and joins its thread before destroying its own
holder at `0x35f74`. VSync removal follows at `0x25190`, and base
destruction follows at `0x251f4`.

Deleting the local holder first does not itself delete the worker's
separate interpreter. The [pending-stop ownership edge](predictor-worker-findings.md#stop-and-model-changes-do-not-inherently-drain-pending-work)
still applies. Concurrent inline calls and callbacks require outer lifetime
coordination that these destructor instructions alone do not establish.

Holder destruction recursively destroys its input-type map through
`0x37ff4`. Per-node cleanup `0x38bcc` destroys validators, then the
interpreter, then the flat-buffer model at `0x38be4`, `0x38bf0`,
and `0x38c04`. This final destruction order differs from `AddModel`'s
replacement order, which clears the flat-buffer owner before the interpreter.

## Validation and follow-up

Both native byte streams were matched to the APK. Documented instructions,
owner-reset calls, branch destinations, imported interpreter destructor,
TFLite runner storage and deletion, and Markdown links were checked.
Disposable ownership reconstruction covered repeated input-type updates,
five setup outcomes, missing-record preservation, stable map identity,
and stale task bindings after replacement.

No native inference, fault injection, or device lifecycle stress test was
performed. The next useful trace is Composer's outer serialization around
prediction reconfiguration and deletion. This work changes only research
documentation; it supplies no new saved-stroke decoding rule or SDK behavior.
