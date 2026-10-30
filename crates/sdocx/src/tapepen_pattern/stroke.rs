/*
 * Copyright (c) 2013-14 Mikko Mononen memon@inside.org
 *
 * This software is provided 'as-is', without any express or implied
 * warranty.  In no event will the authors be held liable for any damages
 * arising from the use of this software.
 *
 * Permission is granted to anyone to use this software for any purpose,
 * including commercial applications, and to alter it and redistribute it
 * freely, subject to the following restrictions:
 *
 * 1. The origin of this software must not be misrepresented; you must not
 * claim that you wrote the original software. If you use this software
 * in a product, an acknowledgment in the product documentation would be
 * appreciated but is not required.
 * 2. Altered source versions must be plainly marked as such, and must not be
 * misrepresented as being the original software.
 * 3. This notice may not be removed or altered from any source distribution.
 *
 * The polygon rasterization is heavily based on stb_truetype rasterizer
 * by Sean Barrett - http://nothings.org/
 *
 */
//! Altered, restricted NanoSVG translation for a single flat cubic round-capped stroke.
//! This is not the original software and rejects curves and other stroke types.
macro_rules! ensure {
    ($condition:expr $(,)?) => {
        if !$condition {
            return Err(());
        }
    };
}

use super::scan::Edge;

pub fn try_stroke(p: [f32; 8], width: f32, scale: f32) -> Result<Vec<Edge>, ()> {
    ensure!(width.is_finite() && width > 0.0 && scale.is_finite() && scale > 0.0);
    ensure!(p.iter().all(|v| v.is_finite()));
    let p = p.map(|v| v * scale);
    // Keep derived edge coordinates within the scan converter's bounds.
    ensure!(p.iter().all(|v| v.is_finite() && v.abs() <= 1000.0));
    let dx = p[6] - p[0];
    let dy = p[7] - p[1];
    let d2 = ((p[2] - p[6]) * dy - (p[3] - p[7]) * dx).abs();
    let d3 = ((p[4] - p[6]) * dy - (p[5] - p[7]) * dx).abs();
    ensure!((d2 + d3) * (d2 + d3) < 0.25 * (dx * dx + dy * dy));
    let len = (dx * dx + dy * dy).sqrt();
    ensure!(len > 0.01);
    let inv = 1.0 / len;
    let (dx, dy) = (dx * inv, dy * inv);
    let w = width * scale * 0.5;
    ensure!(w.is_finite() && w > 0.0 && w <= 1000.0);
    let divisions = (std::f32::consts::PI / ((w / (w + 0.25)).acos() * 2.0))
        .ceil()
        .max(2.0);
    ensure!(divisions.is_finite() && divisions <= 128.0);
    let n = divisions as usize;
    let mut edges = Vec::new();
    fn add(edges: &mut Vec<Edge>, a: (f32, f32), b: (f32, f32)) {
        if a.1 == b.1 {
            return;
        }
        let (a, b, dir) = if a.1 < b.1 { (a, b, 1) } else { (b, a, -1) };
        edges.push(Edge {
            x0: a.0,
            y0: a.1 * 5.0,
            x1: b.0,
            y1: b.1 * 5.0,
            dir,
        });
    }
    fn cap(
        edges: &mut Vec<Edge>,
        p: (f32, f32),
        dx: f32,
        dy: f32,
        w: f32,
        n: usize,
    ) -> ((f32, f32), (f32, f32)) {
        let (dlx, dly) = (dy, -dx);
        let mut first = (0.0, 0.0);
        let mut prev = first;
        for i in 0..n {
            let a = i as f32 / (n - 1) as f32 * std::f32::consts::PI;
            let (ax, ay) = (a.cos() * w, a.sin() * w);
            let point = (p.0 - dlx * ax - dx * ay, p.1 - dly * ax - dy * ay);
            if i == 0 {
                first = point;
            } else {
                add(edges, prev, point);
            }
            prev = point;
        }
        (first, prev)
    }
    let (left, right) = cap(&mut edges, (p[0], p[1]), dx, dy, w, n);
    let (end_right, end_left) = cap(&mut edges, (p[6], p[7]), -dx, -dy, w, n);
    add(&mut edges, right, end_right);
    add(&mut edges, end_left, left);
    edges.sort_by(|a, b| a.y0.partial_cmp(&b.y0).unwrap());
    Ok(edges)
}

#[cfg(test)]
mod tests {
    use super::try_stroke;

    const LINE: [f32; 8] = [0.0, 0.0, 2.0, 0.0, 4.0, 0.0, 6.0, 0.0];

    #[test]
    fn rejects_invalid_and_unbounded_inputs() {
        for (path, width, scale) in [
            (LINE, f32::NAN, 1.0),
            (LINE, f32::INFINITY, 1.0),
            (LINE, 0.0, 1.0),
            (LINE, -1.0, 1.0),
            (LINE, 1.0, 0.0),
            (LINE, 1.0, f32::INFINITY),
            (LINE, 1.0, f32::NAN),
            (LINE, f32::MAX, 1.0),
            (LINE, 1.0, f32::MAX),
            ([f32::NAN; 8], 1.0, 1.0),
            ([f32::MAX; 8], 1.0, 1.0),
            (LINE, f32::MIN_POSITIVE, f32::MIN_POSITIVE),
        ] {
            assert!(try_stroke(path, width, scale).is_err());
        }
    }

    #[test]
    fn accepts_bounded_line() {
        assert!(!try_stroke(LINE, 1.7, 1.0).unwrap().is_empty());
    }
}
