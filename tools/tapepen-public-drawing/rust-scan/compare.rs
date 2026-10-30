mod scan;
use std::io::{self, BufRead};
fn main() {
    let input = io::stdin().lock();
    let mut lines = input.lines();
    let mut cases = 0;
    while let Some(line) = lines.next() {
        let line = line.unwrap();
        let mut values = line.split_whitespace();
        let n: usize = values.next().unwrap().parse().unwrap();
        let count: usize = values.next().unwrap().parse().unwrap();
        let mut edges = Vec::new();
        for _ in 0..count {
            edges.push(scan::Edge { x0:values.next().unwrap().parse().unwrap(), y0:values.next().unwrap().parse().unwrap(), x1:values.next().unwrap().parse().unwrap(), y1:values.next().unwrap().parse().unwrap(), dir:values.next().unwrap().parse().unwrap() });
        }
        assert!(values.next().is_none());
        let expected = lines.next().unwrap().unwrap();
        let actual = scan::coverage(&edges,n,n);
        assert_eq!(expected.len(), actual.len()*2);
        for (i, pixel) in actual.iter().enumerate() {
            assert_eq!(*pixel,u8::from_str_radix(&expected[2*i..2*i+2],16).unwrap(),"case {cases} pixel {i}");
        }
        cases += 1;
    }
    assert_eq!(cases, 136, "expected all 136 source-derived strokes");
    println!("{cases} source-derived stroke coverages match upstream NanoSVG");
}
