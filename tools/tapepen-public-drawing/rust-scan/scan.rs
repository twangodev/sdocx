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
//! Partial Rust translation of NanoSVG's rasterizeSortedEdges, addActive,
//! fillActiveEdges and fillScanline. Not the original software. This module
//! consumes already flattened, sorted edges; it is not an SVG rasterizer.

#[derive(Clone, Copy, Debug)]
pub struct Edge { pub x0: f32, pub y0: f32, pub x1: f32, pub y1: f32, pub dir: i32 }
#[derive(Clone, Copy)]
struct Active { x: i32, dx: i32, end: f32, dir: i32 }

/// Coverage for sorted edges whose y coordinates are already scaled by five.
/// Rejects dimensions or edge coordinates outside the bounded experiment domain.
/// The slope bound keeps fixed-point advancement inside the i32 range.
pub fn coverage(edges: &[Edge], width: usize, height: usize) -> Vec<u8> {
    assert!(width <= 1024 && height <= 1024);
    assert!(edges.iter().all(|e| e.x0.is_finite() && e.x1.is_finite() && e.y0.is_finite() && e.y1.is_finite() && e.y0 < e.y1 && (e.dir == 1 || e.dir == -1)));
    let max_slope = 1_000_000.0_f64 / (height as f64 * 5.0 + 1.0);
    for e in edges {
        assert!(e.x0.abs() <= 10_000.0 && e.x1.abs() <= 10_000.0
            && e.y0.abs() <= 10_000.0 && e.y1.abs() <= 10_000.0);
        let dy = f64::from(e.y1) - f64::from(e.y0);
        let slope = (f64::from(e.x1) - f64::from(e.x0)) / dy;
        assert!(dy >= 0.0001 && slope.abs() <= max_slope);
    }
    assert!(edges.windows(2).all(|pair| pair[0].y0 <= pair[1].y0));
    let mut result = vec![0; width.checked_mul(height).unwrap()];
    let mut active: Vec<Active> = Vec::new();
    let mut next = 0;
    for y in 0..height {
        let row = &mut result[y * width..(y + 1) * width];
        for sample in 0..5 {
            let scan_y = (y * 5 + sample) as f32 + 0.5;
            active.retain(|edge| edge.end > scan_y);
            for edge in &mut active { edge.x = edge.x.checked_add(edge.dx).expect("fixed-point edge overflow"); }
            active.sort_by_key(|edge| edge.x);
            while next < edges.len() && edges[next].y0 <= scan_y {
                let e = edges[next];
                if e.y1 > scan_y {
                    let slope = (e.x1 - e.x0) / (e.y1 - e.y0);
                    let edge = Active { x: (1024.0 * (e.x0 + slope * (scan_y - e.y0))).round() as i32,
                        dx: (1024.0 * slope).round() as i32, end: e.y1, dir: e.dir };
                    // NanoSVG inserts after the first equal edge and before subsequent equals.
                    let pos = if active.is_empty() || edge.x < active[0].x { 0 } else {
                        1 + active[1..].partition_point(|other| other.x < edge.x)
                    };
                    active.insert(pos, edge);
                }
                next += 1;
            }
            let mut winding = 0;
            let mut start = 0;
            for edge in &active {
                if winding == 0 { start = edge.x; }
                winding += edge.dir;
                if winding == 0 { fill(row, start, edge.x); }
            }
        }
    }
    result
}
fn fill(row: &mut [u8], x0: i32, x1: i32) {
    let mut i = x0 >> 10;
    let mut j = x1 >> 10;
    let len = row.len() as i32;
    if i >= len || j < 0 { return; }
    let add = |row: &mut [u8], at: i32, amount: i32| {
        row[at as usize] = row[at as usize].wrapping_add(amount as u8);
    };
    if i == j { add(row, i, (x1 - x0) * 51 >> 10); }
    else {
        if i >= 0 { add(row, i, ((1024 - (x0 & 1023)) * 51) >> 10); } else { i = -1; }
        if j < len { add(row, j, ((x1 & 1023) * 51) >> 10); } else { j = len; }
        for x in i + 1..j { add(row, x, 51); }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_extreme_edges() {
        let valid = Edge {x0:0.0,y0:0.0,x1:1.0,y1:1.0,dir:1};
        for edge in [Edge {x0:f32::MAX,..valid}, Edge {y1: f32::MIN_POSITIVE,..valid}, Edge {x1:10_000.0,y1:0.0001,..valid}] {
            assert!(std::panic::catch_unwind(|| coverage(&[edge], 64, 64)).is_err());
        }
        assert!(std::panic::catch_unwind(|| coverage(&[valid], 1025, 1)).is_err());
    }
    #[test]
    fn rectangle_and_clipped_rectangle() {
        let edges = [Edge{x0:0.25,y0:0.0,x1:0.25,y1:10.0,dir:1}, Edge{x0:1.75,y0:0.0,x1:1.75,y1:10.0,dir:-1}];
        assert_eq!(coverage(&edges, 3, 2), [190,190,0,190,190,0]);
        let clipped = [Edge{x0:-1.0,..edges[0]},Edge{x0:4.0,x1:4.0,..edges[1]}];
        assert_eq!(coverage(&clipped, 2, 1), [255,255]);
    }
}
