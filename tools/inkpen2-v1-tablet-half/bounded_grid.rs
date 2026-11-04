// Test-only bottom-left pixel grid model; no production export contract.
use std::collections::BTreeMap;

fn add_work(work: usize, width: usize, height: usize) -> Option<usize> {
    work.checked_add(width.checked_mul(height)?)
}

pub fn grid(data: &[u8], width: i32, height: i32, scale: f64, tx: f64, ty: f64, limits: [usize; 3]) -> Option<BTreeMap<(i32, i32), u8>> {
    if width <= 0 || height <= 0 || !scale.is_finite() || scale <= 0.0 || !tx.is_finite() || !ty.is_finite() || !data.len().is_multiple_of(12) || data.len() / 12 > limits[0] { return None; }
    let mut pixels = BTreeMap::new();
    let mut work = 0usize;
    for stamp in data.as_chunks::<12>().0 {
        let f = |i| f64::from(f32::from_le_bytes(stamp[i..i+4].try_into().unwrap()));
        let (x, y, radius) = (f(0), f(4), f(8));
        let (cx, cy, r) = (tx + x * scale, ty + y * scale, radius * scale);
        if ![x,y,radius,cx,cy,r].iter().all(|v| v.is_finite()) { return None; }
        if r <= 0.0 { continue; }
        let bound = |v: f64, max: i32| v.ceil().clamp(0.0, f64::from(max)) as i32;
        let (x0,x1,y0,y1) = (bound(cx-r-0.5,width),bound(cx+r-0.5,width),bound(cy-r-0.5,height),bound(cy+r-0.5,height));
        work = add_work(work, (x1-x0) as usize, (y1-y0) as usize)?;
        if work > limits[1] { return None; }
        let distance = |px: i32, py: i32| ((f64::from(px)+0.5-cx).hypot(f64::from(py)+0.5-cy))/(2.0*r);
        for py in y0..y1 { for px in x0..x1 {
            let (qx,qy) = (px & !1,py & !1);
            let d = distance(px,py);
            let w = (distance(qx+1,py)-distance(qx,py)).abs() + (distance(px,qy+1)-distance(px,qy)).abs();
            let a = if w == 0.0 { 0.0 } else { ((0.5-d)/w).clamp(0.0,1.0) };
            let previous = f64::from(*pixels.get(&(px,py)).unwrap_or(&0));
            let value = (255.0*a + previous*(1.0-a)).round_ties_even() as u8;
            if value != 0 { pixels.insert((px,py),value); if pixels.len() > limits[2] { return None; } }
        }}
    }
    Some(pixels)
}
#[cfg(test)]
mod tests {
    fn stamp(x: f32, y: f32, radius: f32) -> Vec<u8> {
        [x, y, radius].into_iter().flat_map(f32::to_le_bytes).collect()
    }

    #[test]
    fn work_overflow_is_rejected() {
        assert_eq!(super::add_work(0, usize::MAX, 2), None);
        assert_eq!(super::add_work(usize::MAX, 1, 1), None);
        assert_eq!(super::add_work(3, 4, 5), Some(23));
    }

    #[test]
    fn clipping_agrees_with_a_larger_grid_on_the_same_pixel_phase() {
        let data = stamp(0.0, 0.0, 8.0);
        let limits = [1, 1000, 1000];
        let clipped = super::grid(&data, 4, 4, 0.5, 0.0, 0.0, limits).unwrap();
        let larger = super::grid(&data, 12, 12, 0.5, 4.0, 4.0, limits).unwrap();
        let expected = larger.into_iter().filter_map(|((x, y), a)| {
            ((4..8).contains(&x) && (4..8).contains(&y)).then_some(((x - 4, y - 4), a))
        }).collect();
        assert_eq!(clipped, expected);
    }

    #[test]
    fn invalid_inputs_and_each_budget_fail_without_partial_output() {
        let data = stamp(4.0, 4.0, 8.0);
        let valid = [1, 1000, 1000];
        assert!(super::grid(&data, 8, 8, 0.5, 0.0, 0.0, valid).is_some());
        for limits in [[0, 1000, 1000], [1, 0, 1000], [1, 1000, 0]] {
            assert!(super::grid(&data, 8, 8, 0.5, 0.0, 0.0, limits).is_none());
        }
        assert!(super::grid(&stamp(f32::NAN, 0.0, 1.0), 8, 8, 0.5, 0.0, 0.0, valid).is_none());
        assert!(super::grid(&data, 8, 8, 0.5, f64::INFINITY, 0.0, valid).is_none());
        assert!(super::grid(&data[..11], 8, 8, 0.5, 0.0, 0.0, valid).is_none());
    }
}
