mod scan;
mod stroke;
mod texture;
use std::io::{self, BufRead, Write};
fn main() {
    let mut lines = io::stdin().lock().lines();
    let mut image = Vec::new();
    let mut size = 0;
    let mut count = 0;
    while let Some(line) = lines.next() {
        let line = line.unwrap();
        let mut v = line.split_whitespace();
        let n: usize = v.next().unwrap().parse().unwrap();
        let edges: usize = v.next().unwrap().parse().unwrap();
        let color: u32 = v.next().unwrap().parse().unwrap();
        let opacity: f32 = v.next().unwrap().parse().unwrap();
        let kind: u8 = v.next().unwrap().parse().unwrap();
        let stroke_input = if kind == 1 {
            let width = v.next().unwrap().parse().unwrap();
            let mut p = [0.0; 8];
            for x in &mut p {
                *x = v.next().unwrap().parse().unwrap();
            }
            Some((p, width))
        } else {
            None
        };
        if size == 0 {
            size = n;
            image.resize(n * n * 4, 0);
        } else {
            assert_eq!(size, n);
        }
        let mut paths = Vec::new();
        for _ in 0..edges {
            paths.push(scan::Edge {
                x0: v.next().unwrap().parse().unwrap(),
                y0: v.next().unwrap().parse().unwrap(),
                x1: v.next().unwrap().parse().unwrap(),
                y1: v.next().unwrap().parse().unwrap(),
                dir: v.next().unwrap().parse().unwrap(),
            });
        }
        assert!(v.next().is_none());
        if let Some((p, width)) = stroke_input {
            let derived = stroke::stroke(p, width, n as f32 / 120.0);
            assert_eq!(paths.len(), derived.len());
            for (actual, expected) in derived.iter().zip(&paths) {
                assert_eq!(
                    (
                        actual.x0.to_bits(),
                        actual.y0.to_bits(),
                        actual.x1.to_bits(),
                        actual.y1.to_bits(),
                        actual.dir
                    ),
                    (
                        expected.x0.to_bits(),
                        expected.y0.to_bits(),
                        expected.x1.to_bits(),
                        expected.y1.to_bits(),
                        expected.dir
                    ),
                    "edge mismatch: {actual:?} != {expected:?}"
                );
            }
            paths = derived;
        }
        let expected = lines.next().unwrap().unwrap();
        let coverage = scan::coverage(&paths, n, n);
        assert_eq!(expected.len(), coverage.len() * 2);
        for (i, p) in coverage.iter().enumerate() {
            assert_eq!(
                *p,
                u8::from_str_radix(&expected[2 * i..2 * i + 2], 16).unwrap()
            );
        }
        texture::blend(&mut image, &coverage, color, opacity);
        count += 1;
    }
    assert!(count > 0);
    texture::finish(&mut image, size, size);
    io::stdout().lock().write_all(&image).unwrap();
}
