# Device policy for external prediction

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so`, `libSPenEngine.so`, `libSPenBase.so`, and
`libSPenPredictor.so` from the [identified APK](README.md#sources-and-validation).

Composer obtains the Boolean controlling worker construction from a
device-model policy. A separate model-prefix check affects the proxy's
reported predictor kind. These checks use different lists and have
different consumers. They do not establish a saved-stroke format change
or the actual configuration of a captured device.

## The low-latency view obtains the worker flag from Engine

`LowLatencyStrokeView` construction at Composer `0x4d1eb8` calls its
own virtual slot 312 at `0x4d1f98`, then passes the result's low bit
as the Boolean argument to `TouchPresenter` at `0x4d1fa8`.
GOT `0x5a2bc0` resolves to vtable `0x580c68`, with primary address
point `0x580c78`; slot 312 resolves to `0x4d4920`.

The signature string at Composer `0x1f23d6` identifies that method as
`LowLatencyStrokeView::IsDeviceAsyncNeeded`. It calls
`LatencyConfigurationFactory::GetInstance` through Composer PLT
`0x559ca0`, relocation `0x5a9248`, then invokes configuration slot
72 at `0x4d4938`.

The factory is Engine `0xc4a40`. It returns the singleton at
`0x194000`, constructing it through `0xf9cb8` on first access.
Engine GOT `0x18bdd8` supplies vtable `0x17bb40`, primary address
point `0x17bb50`, and RTTI names `SPen::LatencyConfiguration`.
Its slot 72 resolves to Engine `0xf9a50`.

## Worker construction requires a matching prefix and SDK value other than 29

Engine `0xf9a50` passes the list at `0x193530` to helper `0xf994c`.
The relocation-resolved list consists of one string pointer followed
by null:

| List element | Value |
| --- | --- |
| `0x193530` | String `0x56e56`, `SM-P61` |
| `0x193538` | Null terminator |

The helper reads property `ro.product.model`, string Engine `0x61d6e`,
through `System::GetAndroidSystemProperty` at `0xf997c`. For each
nonnull list entry it obtains the prefix length with `strlen` at
`0xf9994`, then calls `strncmp(property, prefix, length)` at
`0xf99a4`.

The helper returns false for a prefix match at `0xf99fc`; it returns
true when the property read yields zero, the list is empty, or no prefix
matches at `0xf99b4`. Its diagnostic string at `0x64197` names the
argument `modelBlockList`. That name does not determine the final policy:
the caller interprets the return value.

Configuration slot 72 returns false when the helper returns true.
Only after a prefix match does it call `System::GetSDKVersion` at
Engine `0xf9a68`. It also returns false when that value equals 29
at `0xf9a6c`–`0xf9a78`; otherwise it returns true at `0xf9a94`.
The reconstructed policy is:

```text
worker_requested = property_available
                   and model_name.starts_with("SM-P61")
                   and cached_sdk_value != 29
```

Matching is case-sensitive and checks only the six-byte prefix. It
accepts the exact string `SM-P61` and strings beginning with it; it is
not an exact full-model comparison. The SDK check is inequality with 29,
not a minimum-version comparison.

The SDK accessor is Base `0xc7588`, a load from `0xf6c34`.
`System::SetSDKVersion`, Base `0xc757c`, writes that same cached value.
This trace does not establish the initialization caller or the value on
a device, so the reconstructed predicate names it explicitly as cached
SDK state.

## The constructor copies this decision into the predictor proxy

`TouchPresenter` saves its Boolean argument at Composer `0x4d6f50`.
It normalizes the low bit at `0x4d7068` and stores it in proxy byte
33 at `0x4d7078`. Later factory creation loads byte 33 at
`0x4dab44` and passes its nonzero test as the third integer argument
at `0x4dab5c`/`0x4dab68`.

Predictor `CreatePredictor` forwards that Boolean to `NNPredictor` at
`0x35a38`/`0x35a3c`. A true argument creates the worker; false
creates only the predictor-local holder. The value is captured when
this low-latency view constructs its presenter. These forwarding paths
do not re-query the model property when each prediction is submitted.

Worker existence still does not force worker execution. The
[execution-route trace](predictor-worker-findings.md#unbuffered-dispatch-selects-inline-execution)
shows that unbuffered dispatch runs inference inline even when a worker
exists. Other users of the exported factory can supply their own Boolean;
this device policy belongs to the traced Composer construction path.

## A separate prefix masks the proxy's reported kind

During proxy construction, Composer calls helper `0x4da838` at
`0x4d708c` and stores its Boolean result in proxy byte 32 at
`0x4d7098`. This helper reads the same `ro.product.model` property,
but uses the list at Composer `0x5b2588`:

| List element | Value |
| --- | --- |
| `0x5b2588` | String `0x1e9a79`, `SM-T39` |
| `0x5b2590` | Null terminator |

Its `strlen`/`strncmp` calls are at `0x4da898`/`0x4da8a8`.
A match returns true through `0x4da8b0`/`0x4da938`. Missing property,
empty list, and no match return false. This helper therefore has the
opposite match-result polarity from Engine's intermediate helper.

Proxy kind getter `0x4daf00` returns zero whenever byte 32 is set.
Otherwise it forwards concrete predictor slot 80 when a predictor exists,
or returns zero for a null pointer. It does not delete an existing
predictor. Proxy input dispatch at `0x4dad78` also does not test byte
32 before forwarding to a concrete predictor.

The [selection setter](predictor-reconfiguration-findings.md#changed-selection-destroys-the-old-predictor-before-creating-a-new-one)
uses this reported kind in its early-return comparison. A masked proxy
with a concrete neural predictor consequently does not report kind 1
for that comparison. This is a getter-level policy, not evidence that
all inference paths are disabled for that prefix.

## Property lookup and concrete boundary examples

Both libraries import Base `System::GetAndroidSystemProperty`,
`0xc75fc`. It resolves `__system_property_get` from `libc.so`, using
strings Base `0x2dbbf` and `0x32716`, caches the function pointer,
and forwards the call at `0xc7698`. Resolution failure clears the
output's first byte and returns zero at `0xc76c4`/`0xc76c8`.
The two policy callers test whether the returned value is zero.

With a successful property lookup and stable property value between
the separate constructor queries:

| Model string | Cached SDK | Worker requested | Proxy kind masked |
| --- | --- | --- | --- |
| `SM-P610` | 29 | No | No |
| `SM-P610` | 30 | Yes | No |
| `SM-P61` | 28 | Yes | No |
| `sm-p610` | 30 | No | No |
| `SM-P620` | 30 | No | No |
| `SM-T390` | 30 | No | Yes |
| `SM-T39` | 29 | No | Yes |

These strings are predicate examples, not a verified product catalog.
A missing property yields false for both final flags. No Android property
was changed or queried from an actual device for this reconstruction.

## Validation and remaining work

All four native byte streams were matched to the APK. Relocated prefix
lists, null terminators, property strings, factory and vtable targets,
imported string/property/SDK functions, and documented instructions were
checked. Disposable comparisons covered exact prefixes, longer matches,
shorter nonmatches, case changes, absent property, and SDK values below,
equal to, and above 29.

This narrows the worker construction path without establishing the active
device model, cached SDK initialization, selected predictor, or unbuffered
mode. Callback queue ownership and delivery remain separate work.
No SDK code or corpus fixture changed.
