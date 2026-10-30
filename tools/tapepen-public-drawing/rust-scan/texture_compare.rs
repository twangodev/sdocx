mod scan;
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
