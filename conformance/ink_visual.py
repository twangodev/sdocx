"""Measure ink only inside the union of prepared stroke bounds.

The geometry JSON comes from the ink_geometry example for a single-page fixture.
Reference and candidate images must have the same dimensions and use one image
pixel per geometry unit. Three pixels of padding retain antialias coverage.
This is a stroke-region measurement, not isolation of overlapping page objects.
"""
import argparse
import json
import math
from pathlib import Path

import numpy as np
from PIL import Image
from visual import measure, rgb_on_white


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("geometry", type=Path)
    parser.add_argument("reference", type=Path)
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()
    reference = rgb_on_white(Image.open(args.reference))
    candidate = rgb_on_white(Image.open(args.candidate))
    if reference.size != candidate.size:
        raise ValueError("images must have matching dimensions")
    width, height = reference.size
    mask = np.zeros((height, width), dtype=bool)
    count = 0
    for item in json.loads(args.geometry.read_text()):
        bounds = item["geometry"]["bounds"]
        if not bounds:
            continue
        x0, y0 = math.floor(bounds["x_min"]) - 3, math.floor(bounds["y_min"]) - 3
        x1, y1 = math.ceil(bounds["x_max"]) + 3, math.ceil(bounds["y_max"]) + 3
        if x1 < 0 or y1 < 0 or x0 >= width or y0 >= height:
            raise ValueError("stroke bounds outside image; check page and coordinate scale")
        mask[max(0, y0):min(height, y1), max(0, x0):min(width, x1)] = True
        count += 1
    if not count:
        raise ValueError("no prepared stroke bounds")
    def masked(image):
        pixels = np.asarray(image).copy()
        pixels[~mask] = 255
        return Image.fromarray(pixels)
    metrics, _ = measure(masked(reference), masked(candidate))
    ink = {key: value for key, value in metrics.items() if "ink" in key}
    print(json.dumps({"stroke_regions": count, "padding": 3, "tolerance": 1,
                      "ink_threshold": 32, **ink}, indent=2))


if __name__ == "__main__":
    main()
