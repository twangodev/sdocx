"""Pure checks for retaining and running captured native queue entries."""

from __future__ import annotations

import struct


def validate_task_snapshot(row, live_bytes, live_run, expected, queue):
    """Validate recorded ownership and immutable task bytes before native Run."""
    raw = bytes.fromhex(row["task_bytes_at_submission"])
    if (not row.get("accepted") or not row.get("ownership_transferred_to_queue")
            or row.get("run_state") != "deferred_not_run"
            or int(row.get("queue", "0"), 16) != queue
            or len(raw) != row.get("allocation_size")
            or bytes(live_bytes) != raw
            or struct.unpack_from("<Q", raw)[0] != int(row["task_vptr"], 16)
            or int(live_run) != int(row["run_method"], 16)):
        raise ValueError("native task ownership, queue, snapshot, vptr, or Run entry changed")
    for key, value in expected.items():
        if key in ("member_method", "identity", "receiver"):
            continue
        if row.get(key) != value:
            raise ValueError(f"native task field {key} differs from source witness")
    if "member_method" in expected:
        member = int(expected["member_method"])
        if len(raw) < 32 or struct.unpack_from("<Q", raw, 24)[0] != member:
            raise ValueError("native task member pointer differs from source witness")
        if row.get("member_method_pointer") != hex(member):
            raise ValueError("native task member pointer metadata differs from source witness")
    if expected.get("method_symbol") is not None:
        resolution = row.get("member_method_resolution")
        if (row.get("method_symbol") != expected["method_symbol"]
                or not isinstance(resolution, dict)
                or resolution.get("symbol") != expected["method_symbol"]
                or resolution.get("address") != hex(int(expected["member_method"]))
                or not resolution.get("dso_sha256")):
            raise ValueError("native task member pointer lacks exact pinned symbol resolution")
    return True
