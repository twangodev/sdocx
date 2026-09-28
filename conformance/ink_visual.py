"""Measure ink only inside the union of prepared stroke bounds.

The geometry JSON comes from the ink_geometry example for a single-page fixture.
Reference and candidate images must have the same dimensions and use one image
pixel per geometry unit. Three pixels of padding retain antialias coverage.
Profile/settings filters select regions, not individual overlapping objects.
For colored pens, chroma selection can exclude monochrome ink. Compare isolated
layers when available; a region mask alone cannot undo compositing.
"""
import argparse
import json
import math
from pathlib import Path

import numpy as np
from PIL import Image
from visual import measure, rgb_on_white


def stroke_regions(rows, size, profile=None, settings=None, region=None):
    width, height = size
    mask = np.zeros((height, width), dtype=bool)
    count = 0
    if region is not None:
        x0, y0, x1, y1 = region
        if not (0 <= x0 < x1 <= width and 0 <= y0 < y1 <= height):
            raise ValueError('region must be a nonempty rectangle inside the image')
    for item in rows:
        if profile is not None and item['geometry'].get('profile') != profile:
            continue
        rendering = item['stroke'].get('rendering') or {}
        if settings is not None and rendering.get('advanced_settings') != settings:
            continue
        bounds = item['geometry']['bounds']
        if not bounds:
            continue
        x0, y0 = math.floor(bounds['x_min']) - 3, math.floor(bounds['y_min']) - 3
        x1, y1 = math.ceil(bounds['x_max']) + 3, math.ceil(bounds['y_max']) + 3
        if region is not None:
            x0, y0 = max(x0, region[0]), max(y0, region[1])
            x1, y1 = min(x1, region[2]), min(y1, region[3])
            if x0 >= x1 or y0 >= y1:
                continue
        if x1 < 0 or y1 < 0 or x0 >= width or y0 >= height:
            raise ValueError('stroke bounds outside image; check page and coordinate scale')
        mask[max(0, y0):min(height, y1), max(0, x0):min(width, x1)] = True
        count += 1
    if not count:
        raise ValueError('no selected stroke regions')
    return mask, count


def masked_ink(image, mask, chroma_threshold=None):
    pixels = np.asarray(rgb_on_white(image)).copy()
    selected = mask.copy()
    if chroma_threshold is not None:
        if not 0 <= chroma_threshold < 255:
            raise ValueError('chroma threshold must be in 0..254')
        selected &= np.ptp(pixels, axis=2) > chroma_threshold
    pixels[~selected] = 255
    return Image.fromarray(pixels)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("geometry", type=Path)
    parser.add_argument("reference", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument('--profile', help='prepared pen profile, e.g. Marker4')
    parser.add_argument('--settings', help='exact saved settings, e.g. 7;')
    parser.add_argument('--region', nargs=4, type=int, metavar=('LEFT', 'TOP', 'RIGHT', 'BOTTOM'))
    parser.add_argument('--chroma-threshold', type=int, help='isolate colored ink by max RGB minus min RGB')
    args = parser.parse_args()
    reference = rgb_on_white(Image.open(args.reference))
    candidate = rgb_on_white(Image.open(args.candidate))
    if reference.size != candidate.size:
        raise ValueError("images must have matching dimensions")
    mask, count = stroke_regions(json.loads(args.geometry.read_text()), reference.size,
                                 args.profile, args.settings, args.region)
    ref = masked_ink(reference, mask, args.chroma_threshold)
    sdk = masked_ink(candidate, mask, args.chroma_threshold)
    metrics, _ = measure(ref, sdk)
    a, b = np.asarray(ref), np.asarray(sdk)
    common = (a.min(axis=2) < 223) & (b.min(axis=2) < 223)
    union = (a.min(axis=2) < 223) | (b.min(axis=2) < 223)
    common_rgb_error = float(np.abs(a.astype(float) - b).mean(axis=2)[common].mean()) if common.any() else None
    ink = {key: value for key, value in metrics.items() if "ink" in key}
    print(json.dumps({"stroke_regions": count, "padding": 3, "tolerance": 1,
                      "ink_threshold": 32, 'profile': args.profile, 'settings': args.settings,
                      'region': args.region, 'chroma_threshold': args.chroma_threshold,
                      'ink_intersection_over_union': float(common.sum() / union.sum()) if union.any() else None,
                      'shared_ink_mean_rgb_error': common_rgb_error, **ink}, indent=2))


if __name__ == "__main__":
    main()
