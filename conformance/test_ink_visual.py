"""Checks that selected-pen comparisons cannot silently measure other ink."""
import unittest

import numpy as np
from PIL import Image

from ink_visual import masked_ink, stroke_regions


def stroke(profile, settings, x, y):
    return {'stroke': {'rendering': {'advanced_settings': settings}},
            'geometry': {'profile': profile,
                         'bounds': {'x_min': x, 'y_min': y, 'x_max': x + 2, 'y_max': y + 2}}}


class InkVisualTests(unittest.TestCase):
    def test_profile_settings_and_region_select_only_requested_strokes(self):
        rows = [stroke('Marker4', '7;', 10, 10), stroke('Marker4', '8;', 30, 10),
                stroke('FountainPen', '14;', 10, 30), stroke('Marker4', '7;', 10, 100)]
        mask, count = stroke_regions(rows, (50, 50), 'Marker4', '7;', (0, 0, 50, 20))
        self.assertEqual(count, 1)
        self.assertTrue(mask[10, 10])
        self.assertFalse(mask[10, 30])
        self.assertFalse(mask[30, 10])
        with self.assertRaisesRegex(ValueError, 'outside image'):
            stroke_regions(rows, (50, 50), 'Marker4', '7;')
        with self.assertRaisesRegex(ValueError, 'no selected'):
            stroke_regions(rows, (50, 50), 'InkPen2')

    def test_invalid_regions_are_rejected(self):
        for region in [(0, 0, 0, 10), (-1, 0, 20, 20), (0, 0, 51, 50)]:
            with self.assertRaisesRegex(ValueError, 'region'):
                stroke_regions([], (50, 50), region=region)

    def test_chroma_selection_removes_monochrome_and_composites_transparency(self):
        pixels = np.array([[[0, 0, 0, 255], [50, 150, 255, 255], [50, 150, 255, 0]]], dtype=np.uint8)
        mask = np.ones((1, 3), dtype=bool)
        selected = np.asarray(masked_ink(Image.fromarray(pixels), mask, 35))
        self.assertEqual(selected[0].tolist(), [[255, 255, 255], [50, 150, 255], [255, 255, 255]])
        mask[0, 1] = False
        self.assertTrue((np.asarray(masked_ink(Image.fromarray(pixels), mask, 35)) == 255).all())
        with self.assertRaisesRegex(ValueError, 'chroma'):
            masked_ink(Image.fromarray(pixels), mask, 255)


if __name__ == '__main__':
    unittest.main()
