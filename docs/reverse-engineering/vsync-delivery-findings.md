# VSync delivery to neural prediction

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenEngine.so`, `libSPenComposer.so` and `libSPenPredictor.so`, plus
fresh fallback decompilation of `SpenVSyncProvider` from the
[identified APK](README.md#sources-and-validation).

The neural predictor's stored VSync value comes from the Java
`Choreographer.FrameCallback.doFrame` argument. The traced Java/JNI/
provider handoff forwards that value unchanged. It becomes the alignment
origin described in [prediction callback timing](predictor-timing-findings.md).

## Java forwards the frame callback argument

Fresh decompilation identifies `SpenVSyncProvider` in `classes8.dex`.
Its three methods have these operations:

| Method | Operations in order |
| --- | --- |
| `subscribe()` | `Choreographer.getInstance().postFrameCallback(this)` |
| `doFrame(frameTimeNanos)` | Post the next callback, then forward `frameTimeNanos` through the companion bridge to `Native_SendVSync` |
| `unSubscribe()` | `Choreographer.getInstance().removeFrameCallback(this)` |

The companion and synthetic accessor methods forward the same Java
`long`. There is no clock read or timestamp adjustment in these methods.

Engine `VSyncProviderGlue::OnLoad`, `0xc5edc`, finds class
`com/samsung/android/sdk/pen/engine/SpenVSyncProvider` using string
`0x689d9`. Its one-entry JNI table at `0x192f48` is:

| Name | Signature | Native function |
| --- | --- | --- |
| `Native_SendVSync` | `(J)V` | `0xc5ec4` |

Registration occurs at `0xc5f40`–`0xc5f58`. The same setup resolves
`subscribe` and `unSubscribe`, both `()V`, and saves their method IDs at
glue offsets 16 and 24 through `0xc5f84`/`0xc5fb0`. It constructs a
Java provider object and saves its global reference at glue offset 32
through `0xc6004`.

The global glue object is at `0x1940f0`. Its provider pointer at offset
40 is written at `0xc6048`. `Native_SendVSync`, `0xc5ec4`, loads that
pointer from `0x194118`, moves the Java timestamp from `x2` to `x1`,
and tail-calls `VSyncProvider::DispatchVSync` at `0xc5ed0`.

## The native provider broadcasts the unchanged timestamp

Engine `VSyncProvider::CreateVSyncProvider`, `0xc7a88`, creates the
64-byte singleton and saves it through the pointer resolved by GOT
`0x18ba80` to `VSyncProvider::mInstance`, `0x194168`.
`IVSyncProvider::GetInstance`, `0xc7a78`, reads that same singleton.

The provider constructor, `0xc7b14`, establishes:

| Offset | Role |
| --- | --- |
| 8 | Critical section |
| 24/32/40 | Receiver-vector begin/end/capacity |
| 48 | Glue pointer |
| 56 | Subscription-state byte, initially zero |

`DispatchVSync`, `0xc7b58`, holds that critical section while iterating
the receiver vector. It preserves the supplied timestamp in `x19` at
`0xc7b74` and passes it to every receiver's slot 0 at
`0xc7b98`–`0xc7ba8`. There is no timestamp arithmetic in this dispatch.

## The first receiver subscribes and the last removal unsubscribes

Provider vtable `0x1798f8`, primary address point `0x179908`, binds
slot 16 to `RegisterCallBack`, `0xc7c08`, and slot 24 to
`UnregisterCallBack`, `0xc7dfc`.

Registration searches for the exact receiver pointer at
`0xc7c44`–`0xc7c74`. An existing pointer returns without appending.
After a new pointer is appended, `0xc7d28`–`0xc7d30` checks whether
the vector now contains exactly one pointer. That transition invokes
glue slot 0 at `0xc7d40` and sets subscription byte 56 at `0xc7d4c`.
Glue slot 0 is `SubscribeForVSync`, `0xc5da0`, which calls the saved
Java `subscribe` method at `0xc5ddc`.

Unregistration finds the pointer, shifts later entries with `memmove`
when necessary, and updates the vector end at `0xc7e84`. If the vector
is empty and subscription byte 56 is set, it invokes glue slot 8 at
`0xc7ea8` and clears the byte at `0xc7eb0`. Glue slot 8 is
`UnsubscribeFromVSync`, `0xc5e34`, which calls Java `unSubscribe` at
`0xc5e6c`.

Thus duplicate registration does not add a second receiver or post an
additional subscription through this provider. Removing one of several
receivers leaves the subscription active. Removing an absent pointer
does not erase another receiver.

## Composer supplies registration and removal callbacks

When [creating a predictor](predictor-callback-findings.md), Composer
builds a pair of callbacks and supplies them to predictor slot 48 at
`0x4dac60`–`0x4dac68`:

| Pair offset | Composer callable | Native provider dispatch |
| --- | --- | --- |
| 0 | `0x4db0dc` | Singleton slot 16: register receiver |
| 48 | `0x4db048` | Singleton slot 24: unregister receiver |

The callables' vtable address points are `0x5811c8` and `0x581170`;
their invocation slots are at `0x5811f8` and `0x5811a0`. Both obtain
the singleton through Composer PLT `0x55e860`, whose relocation at
`0x5ab828` names `IVSyncProvider::GetInstance`.

Predictor slot 48 is `NNPredictor::SetVSyncEventCallback`, `0x25324`.
It copies the first callable into neural offset 1808 at `0x25358`–
`0x25360`, and the second into offset 1856 at `0x25364`–`0x25374`.
Their callable pointers are at offsets 1840 and 1888 respectively.

`NNPredictor::Predict`, `0x25258`, selects them by incoming action:

| Action value | Operation before base prediction |
| --- | --- |
| 0 | Clear stored VSync at `0x25314`, then invoke registration if installed |
| 1 or 3 | Invoke removal if installed, through `0x252bc`–`0x252d8` |
| Other values | Continue to base prediction without this registration/removal call |

The receiver argument is the neural object's secondary interface at
offset 1776, formed at `0x252c4`. The destructor also invokes removal
with this interface at `0x25174`–`0x25190` before destroying the saved
callables.

## The secondary interface stores the frame time

The neural constructor installs a secondary vtable address point at
`0x40ac0`. Its offset-to-top is -1776, and its slot 0 is the
`OnVSync` thunk, `0x259cc`.

The thunk stores the supplied timestamp at secondary-interface offset
24 through `0x259f0`: neural-object offset 1800. The primary
`NNPredictor::OnVSync`, `0x259a0`, stores the same field directly at
`0x259c4`. Neither applies an offset or resamples the timestamp.

The complete recovered value path is therefore:

```text
Java doFrame argument
  -> Native_SendVSync(J)V
  -> VSyncProvider::DispatchVSync argument
  -> IVSyncEventReceiver slot 0
  -> NNPredictor member 1800
  -> prediction entity offset 24
```

Clearing member 1800 on action 0 still matters: registration starts
future delivery, while the field remains zero until a frame callback
stores a value. A nonzero prediction period does not prove a VSync
origin has already arrived.

## Validation and remaining work

The APK digest and all three native byte streams were verified. Fresh
class decompilation confirmed the callback argument and repost/removal
operations. JNI registration, Java method-name strings, global pointers,
provider/callable/receiver vtables, subscription branches and timestamp
stores were checked against ARM64 instructions.

This establishes the callback's origin and local lifecycle. It does not
measure the relationship between frame callbacks and physical display
scanout, active device refresh settings, or neural model output. Those
remain separate from this value trace. No SDK code changed and no new
SDOCX fixture or device execution was used.
