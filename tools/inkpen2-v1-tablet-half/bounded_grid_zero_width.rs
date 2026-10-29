// Standalone experimental bottom-left pixel grid; no production export contract.
use std::collections::BTreeMap;
use std::io::{self, Read, Write};

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
            let a = if w == 0.0 {
                if d >= 0.5 { return None; }
                1.0
            } else { ((0.5-d)/w).clamp(0.0,1.0) };
            let previous = f64::from(*pixels.get(&(px,py)).unwrap_or(&0));
            let value = (255.0*a + previous*(1.0-a)).round_ties_even() as u8;
            if value != 0 { pixels.insert((px,py),value); if pixels.len() > limits[2] { return None; } }
        }}
    }
    Some(pixels)
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let dx: f64 = args[0].parse().unwrap(); let dy: f64 = args[1].parse().unwrap();
    let limits: [usize;3] = if args.len() == 5 { [args[2].parse().unwrap(),args[3].parse().unwrap(),args[4].parse().unwrap()] } else { [1000,100000,3072] };
    let mut data = Vec::new(); io::stdin().read_to_end(&mut data).unwrap();
    let Some(pixels) = grid(&data,64,48,0.5,16.0+dx,16.0+dy,limits) else { std::process::exit(2) };
    let mut out = vec![0u8;64*48];
    for ((x,y),a) in pixels { out[y as usize*64+x as usize] = a; }
    io::stdout().write_all(&out).unwrap();
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
    fn odd_viewport_and_slices_keep_the_original_quad_phase() {
        let data = stamp(0.0, 0.0, 16.0);
        let limits = [1, 10000, 10000];
        let viewport = super::grid(&data, 9, 7, 0.5, 8.0, 5.0, limits).unwrap();
        let expanded = super::grid(&data, 14, 12, 0.5, 10.0, 7.0, limits).unwrap();
        let cropped = expanded.into_iter().filter_map(|((x, y), a)| {
            ((2..11).contains(&x) && (2..9).contains(&y)).then_some(((x - 2, y - 2), a))
        }).collect();
        assert_eq!(viewport, cropped);

        // Slice after evaluating the full viewport, so odd slice origins do not
        // change the derivative quad's alignment.
        let lower: std::collections::BTreeMap<_, _> = viewport.iter()
            .filter_map(|(&(x, y), &a)| (y < 3).then_some(((x, y), a))).collect();
        let upper: std::collections::BTreeMap<_, _> = viewport.iter()
            .filter_map(|(&(x, y), &a)| (y >= 3).then_some(((x, y - 3), a))).collect();
        let reconstructed: std::collections::BTreeMap<_, _> = lower.into_iter()
            .chain(upper.into_iter().map(|((x, y), a)| ((x, y + 3), a))).collect();
        assert_eq!(viewport, reconstructed);
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

    #[test]
    fn even_pixel_translation_preserves_quad_phase_but_odd_translation_does_not() {
        let data = stamp(0.0, 0.0, 2.0833335);
        let limits = [1, 1000, 1000];
        let original = super::grid(&data, 32, 32, 0.5, 16.0, 16.0, limits).unwrap();
        let even = super::grid(&data, 32, 32, 0.5, 18.0, 18.0, limits).unwrap();
        let translated = original.iter().map(|(&(x, y), &a)| ((x + 2, y + 2), a)).collect();
        assert_eq!(even, translated);
        let odd = super::grid(&data, 32, 32, 0.5, 17.0, 17.0, limits).unwrap();
        let translated = original.iter().map(|(&(x, y), &a)| ((x + 1, y + 1), a)).collect();
        assert_ne!(odd, translated);
    }

    #[test]
    fn exact_work_and_pixel_budgets_are_accepted() {
        let data = stamp(0.0, 0.0, 2.0833335);
        let expected = super::grid(&data, 32, 32, 0.5, 17.0, 17.0, [1, 1000, 1000]).unwrap();
        assert_eq!(expected.len(), 4);
        assert_eq!(super::grid(&data, 32, 32, 0.5, 17.0, 17.0, [1, 4, 4]), Some(expected));
        assert!(super::grid(&data, 32, 32, 0.5, 17.0, 17.0, [1, 3, 4]).is_none());
        assert!(super::grid(&data, 32, 32, 0.5, 17.0, 17.0, [1, 4, 3]).is_none());
    }

    #[test]
    fn repeated_stamps_charge_cumulative_work() {
        let mut data = stamp(0.0, 0.0, 2.0833335);
        data.extend(stamp(0.0, 0.0, 2.0833335));
        assert!(super::grid(&data, 32, 32, 0.5, 17.0, 17.0, [2, 8, 4]).is_some());
        assert!(super::grid(&data, 32, 32, 0.5, 17.0, 17.0, [2, 7, 4]).is_none());
    }

    #[test]
    fn zero_width_inside_is_opaque_and_outside_is_rejected() {
        let limits = [1, 1000, 1000];
        let inside = super::grid(&stamp(0.0, 0.0, 2.0833335), 32, 32, 0.5, 17.0, 17.0, limits).unwrap();
        for y in 16..18 { for x in 16..18 { assert_eq!(inside.get(&(x, y)), Some(&255)); } }
        assert!(super::grid(&stamp(0.0, 0.0, 1.2), 32, 32, 0.5, 17.0, 17.0, limits).is_none());
    }
}
