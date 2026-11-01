"""Checks for sparse cache writes and the aggregate budget boundary."""
import unittest
from cache_budget import CACHE_BUDGET, within_budget, write_reservation


class CacheBudgetTests(unittest.TestCase):
    def test_sparse_seek_accounts_for_gap(self):
        self.assertEqual(write_reservation(10_000_000, 10, 1), 9_999_993)
        self.assertFalse(within_budget(7_000_000, write_reservation(10_000_000, 10, 1)))

    def test_overwrite_and_budget_boundary(self):
        self.assertEqual(write_reservation(3, 10, 2), 4)
        self.assertTrue(within_budget(CACHE_BUDGET // 2 - 4, 4))
        self.assertFalse(within_budget(CACHE_BUDGET // 2 - 3, 4))


if __name__ == '__main__':
    unittest.main()
