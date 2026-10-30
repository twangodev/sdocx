//! Restricted source-driven rasterizer for the observed 120×120 pattern format.
//! It is not a general SVG parser: every structural element is checked.
mod scan;
mod stroke;
mod texture;
use std::{
    env, fs,
    io::{self, Write},
};

fn color(s: &str) -> u32 {
    assert!(s.len() == 7 && s.starts_with('#'));
    let rgb = u32::from_str_radix(&s[1..], 16).unwrap();
    0xff000000 | ((rgb & 255) << 16) | (rgb & 0xff00) | (rgb >> 16)
}
fn attribute<'a>(s: &'a str, prefix: &str, suffix: &str) -> (&'a str, &'a str) {
    let s = s.strip_prefix(prefix).unwrap();
    s.split_once(suffix).unwrap()
}
fn point(s: &str) -> (f32, f32) {
    let (x, y) = s.split_once(' ').unwrap();
    (x.parse().unwrap(), y.parse().unwrap())
}
fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 3);
    let source = fs::read_to_string(&args[1]).unwrap();
    let size: usize = args[2].parse().unwrap();
    assert!(size == 64 || size == 128);
    let mut lines = source.lines();
    assert_eq!(
        lines.next().unwrap(),
        "<svg width=\"120\" height=\"120\" viewBox=\"0 0 120 120\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">"
    );
    assert_eq!(
        lines.next().unwrap(),
        "<g clip-path=\"url(#clip0_17692_100364)\">"
    );
    let mut image = vec![0; size * size * 4];
    let full = vec![255; size * size];
    let rect = |line: &str| {
        let (c, end) = attribute(line, "<rect width=\"120\" height=\"120\" fill=\"", "\"/>");
        assert!(end.is_empty());
        color(c)
    };
    texture::blend(&mut image, &full, rect(lines.next().unwrap()), 1.0);
    assert_eq!(
        lines.next().unwrap(),
        "<g clip-path=\"url(#clip1_17692_100364)\">"
    );
    texture::blend(&mut image, &full, rect(lines.next().unwrap()), 1.0);
    let mut count = 0;
    loop {
        let line = lines.next().unwrap();
        if line == "</g>" {
            break;
        }
        let (d, rest) = attribute(line, "<path d=\"M", "\" stroke=\"");
        let (c, rest) = rest.split_once("\" stroke-width=\"").unwrap();
        let (w, rest) = rest.split_once("\" stroke-linecap=\"round\"/>").unwrap();
        assert!(rest.is_empty());
        let (start, end) = if let Some((a, b)) = d.split_once('V') {
            let (x, y) = point(a);
            ((x, y), (x, b.parse().unwrap()))
        } else {
            let (a, b) = d.split_once('L').unwrap();
            (point(a), point(b))
        };
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        let p = [
            start.0,
            start.1,
            start.0 + dx / 3.0,
            start.1 + dy / 3.0,
            end.0 - dx / 3.0,
            end.1 - dy / 3.0,
            end.0,
            end.1,
        ];
        let paint = color(c);
        let scale = size as f32 / 120.0;
        assert!(
            p.iter()
                .all(|v| v.is_finite() && (v * scale).abs() <= 1000.0)
        );
        assert!((dx * dx + dy * dy).sqrt() * scale > 0.01);
        let width: f32 = w.parse().unwrap();
        assert!(width.is_finite() && width >= 0.0);
        count += 1;
        if width * scale <= 0.01 {
            continue;
        }
        let edges = stroke::stroke(p, width, scale);
        texture::blend(&mut image, &scan::coverage(&edges, size, size), paint, 1.0);
    }
    assert_eq!(count, 136);
    for expected in [
        "</g>",
        "<defs>",
        "<clipPath id=\"clip0_17692_100364\">",
        "<rect width=\"120\" height=\"120\" fill=\"white\"/>",
        "</clipPath>",
        "<clipPath id=\"clip1_17692_100364\">",
        "<rect width=\"120\" height=\"120\" fill=\"white\"/>",
        "</clipPath>",
        "</defs>",
        "</svg>",
    ] {
        assert_eq!(lines.next().unwrap(), expected);
    }
    assert!(lines.next().is_none());
    texture::finish(&mut image, size, size);
    io::stdout().lock().write_all(&image).unwrap();
}
