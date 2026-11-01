"""Conservative accounting for the native note probe's in-memory cache."""

CACHE_BUDGET = 32 * 1024 * 1024


def write_reservation(cursor, length, count):
    """Reserve stream growth, including a sparse gap, and two input copies."""
    if min(cursor, length, count) < 0:
        raise ValueError('negative cache extent')
    return max(0, cursor + count - length) + 2 * count


def within_budget(live, extra):
    """Allow allocation slack; this bounds cache payload, not process RSS."""
    return live >= 0 and extra >= 0 and 2 * (live + extra) <= CACHE_BUDGET
