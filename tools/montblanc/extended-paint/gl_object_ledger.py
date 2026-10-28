"""Bounded pure helpers for passive host GL-name observation and event joins."""
from __future__ import annotations
import hashlib


def wrap_id_generator(call_name, previous, read_args, read_output_ids, sink, max_count=64):
    """Call the prior GL callback once, then record its actual output-name array.

    Telemetry read/append failures are captured in the row and never replace a
    successful prior callback result or change the prior callback's arguments.
    """
    if not callable(previous):
        raise TypeError("previous GL generator callback must be callable")
    def wrapped():
        count = pointer = None
        row = {"call": call_name}
        try:
            count, pointer = read_args()
            row.update({"count": count, "output_ptr": pointer})
        except Exception as exc:
            row["capture_error"] = f"args:{type(exc).__name__}:{exc}"
        result = previous()
        try:
            if count is None or pointer is None:
                raise ValueError("generator args unavailable")
            if not isinstance(count, int) or count < 0 or count > max_count:
                raise ValueError("generator count outside telemetry bound")
            if count and not pointer:
                raise ValueError("null output pointer for positive count")
            ids = list(read_output_ids(pointer, count)) if count else []
            if len(ids) != count or any(not isinstance(value, int) or value <= 0 for value in ids):
                raise ValueError("generated name output shape/value mismatch")
            row["ids"] = ids
        except Exception as exc:
            row["capture_error"] = f"output:{type(exc).__name__}:{exc}"
        try:
            sink.append(row)
        except Exception:
            pass
        return result
    return wrapped


def generated_ids_since(rows, start, call_name):
    """Read the bounded generation records after a snapshot index."""
    if not isinstance(start, int) or start < 0 or start > len(rows):
        raise ValueError("invalid generation-event snapshot index")
    ids = []
    for row in rows[start:]:
        if row.get("call") != call_name:
            continue
        if row.get("capture_error"):
            raise ValueError(f"incomplete {call_name} output observation")
        values = row.get("ids")
        if not isinstance(values, list) or len(values) != row.get("count"):
            raise ValueError(f"malformed {call_name} output row")
        ids.extend(values)
    return ids


def target_gl_evidence(gl_calls, start, framebuffer, texture, width, height):
    """Join actual adapter event rows for FBO bind, color attach and RGBA8 alloc."""
    if not isinstance(start, int) or start < 0 or start > len(gl_calls):
        raise ValueError("invalid GL event snapshot index")
    rows = gl_calls[start:]
    binds = [r for r in rows if r.get("call") == "glBindFramebuffer" and
             r.get("target") == 0x8D40 and r.get("id") == framebuffer]
    attaches = [r for r in rows if r.get("call") == "glFramebufferTexture2D" and
                r.get("framebuffer") == framebuffer and r.get("args") ==
                [0x8D40, 0x8CE0, 0x0DE1, texture, 0]]
    allocations = [r for r in rows if r.get("call") == "glTexImage2D" and
                   r.get("boundTexture") == texture and r.get("width") == width and
                   r.get("height") == height and r.get("internalFormat") == 0x8058 and
                   r.get("format") == 0x1908 and r.get("type") == 0x1401]
    complete = [r for r in rows if r.get("call") == "glCheckFramebufferStatus" and
                r.get("target") == 0x8D40 and r.get("framebuffer") == framebuffer and
                r.get("status") == 0x8CD5]
    return {"binds": binds, "color_attachments": attaches,
            "rgba8_allocations": allocations, "complete_checks": complete}


def rgba8_readback_sha(data, width, height):
    """Validate a tightly sized RGBA8 host readback and return its digest."""
    expected = width * height * 4
    if not isinstance(data, (bytes, bytearray, memoryview)) or len(data) != expected:
        raise ValueError("RGBA8 readback length does not match dimensions")
    return hashlib.sha256(data).hexdigest()
