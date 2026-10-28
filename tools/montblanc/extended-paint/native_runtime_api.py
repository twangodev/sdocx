"""Narrow adapter for the pinned V2 runtime's common Unicorn return sentinel."""


def common_return_stop(runtime):
    """Read the existing native_drawing_runtime.oc.STOP; never invent a stop PC."""
    oc = getattr(runtime, "oc", None)
    stop = getattr(oc, "STOP", None) if oc is not None else None
    if not isinstance(stop, int) or stop <= 0 or stop & 3:
        raise ValueError("pinned parent runtime has no aligned common return stop")
    return stop
