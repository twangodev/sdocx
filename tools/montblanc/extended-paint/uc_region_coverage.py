"""Pure interval check for Unicorn mem_regions() inclusive endpoints."""
from __future__ import annotations


def region_union_covers(regions, start: int, stop: int) -> bool:
    """Return whether inclusive UC regions cover [start, stop) without gaps.

    Unicorn's mem_regions() reports (begin, end_inclusive, permissions). Adjacent
    mappings may jointly cover one PT_LOAD interval, so no single-region
    containment assumption is made. Invalid ranges are ignored fail-closed.
    """
    if not isinstance(start, int) or not isinstance(stop, int) or start < 0 or stop <= start:
        return False
    spans = []
    for row in regions:
        try:
            begin, end_inclusive = row[0], row[1]
        except (TypeError, IndexError):
            continue
        if not isinstance(begin, int) or not isinstance(end_inclusive, int) or begin < 0 or end_inclusive < begin:
            continue
        left = max(begin, start)
        right = min(end_inclusive + 1, stop)
        if left < right:
            spans.append((left, right))
    spans.sort()
    cursor = start
    for left, right in spans:
        if right <= cursor:
            continue
        if left > cursor:
            return False
        cursor = right
        if cursor >= stop:
            return True
    return False


def region_union_covers_all(regions, base: int, intervals) -> bool:
    """Materialize a potentially one-shot region iterator once for many PT_LOADs."""
    rows = tuple(regions)
    return all(region_union_covers(rows, base + start, base + stop) for start, stop in intervals)
