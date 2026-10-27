//! Bounded host-Mesa device-coordinate shader experiment, not production paint.
const NATIVE_STREAMS: &[u8] = include_bytes!("../../../../../conformance/pencil2-v14-native.bin");
const GRAIN: &[u8] =
    include_bytes!("../../../../../conformance/pencil2-v14-device-profile/grain.r8");
const POINT: &[u8] =
    include_bytes!("../../../../../conformance/pencil2-v14-device-profile/point.r8");

fn sample(tex: &[u8], width: usize, height: usize, u: f32, v: f32, clamp: bool) -> u8 {
    let axis = |q: f32, n: usize| {
        let i = (q * n as f32).floor() as i32;
        if clamp {
            i.clamp(0, n as i32 - 1) as usize
        } else {
            i.rem_euclid(n as i32) as usize
        }
    };
    tex[axis(v, height) * width + axis(u, width)]
}

fn render(scale: f32, translation_x: f32) -> Vec<u8> {
    let mut image = vec![0u8; 192 * 192];
    for row in NATIVE_STREAMS[9408..14112].as_chunks::<56>().0 {
        let mut r = [0f32; 14];
        for (i, value) in r.iter_mut().enumerate() {
            *value = f32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let x0 = (r[0] - r[2]) * scale + translation_x;
        let y0 = 192. - (r[1] + r[3]) * scale;
        let width = 2. * r[2] * scale;
        let height = 2. * r[3] * scale;
        for y in 0..192 {
            for x in 0..192 {
                let fx = (x as f32 + 0.5 - x0) / width;
                let fy = (y as f32 + 0.5 - y0) / height;
                if !(0. ..1.).contains(&fx) || !(0. ..1.).contains(&fy) {
                    continue;
                }
                if sample(POINT, 100, 300, fx, ((1. - fy) + r[6]) / 3., true) == 0 {
                    continue;
                }
                let period = 75. * scale;
                let grain = sample(
                    GRAIN,
                    150,
                    150,
                    (x as f32 + 0.5) / period,
                    (191.5 - y as f32) / period,
                    false,
                ) as f32
                    / 255.;
                let dx = fx - 0.5;
                let dy = 1. - fy - 0.5;
                let dot = dx * r[7] + dy * r[8];
                let p = (dx * dx + dy * dy - dot * dot).sqrt();
                let factor = 1. / (9. * r[5] + 1.);
                if p > factor {
                    continue;
                }
                let mut alpha =
                    (grain * r[4] * ((-(dot + 0.5) + 1.) * ((factor - p) / factor))).clamp(0., 1.);
                let distance = (dx * dx + (fy - 0.5) * (fy - 0.5)).sqrt();
                let mut smooth = r[10];
                if distance > r[9] {
                    smooth *= 1. - (distance - r[9]) / (0.5 - r[9]);
                }
                alpha *= smooth;
                if !alpha.is_finite() {
                    continue;
                }
                alpha = alpha.clamp(0., 1.);
                let i = y * 192 + x;
                image[i] = ((alpha + (image[i] as f32 / 255.) * (1. - alpha)) * 255.)
                    .round()
                    .clamp(0., 255.) as u8;
            }
        }
    }
    image
}

#[test]
fn device_profile_matches_bounded_host_controls() {
    type DeviceCase<'a> = (f32, f32, &'a [u8], usize, u32, u8);
    let cases: [DeviceCase<'_>; 3] = [
        (
            1.,
            0.,
            include_bytes!("../../../../../conformance/pencil2-v14-device-profile/baseline.alpha"),
            22,
            29,
            4,
        ),
        (
            1.,
            6.,
            include_bytes!(
                "../../../../../conformance/pencil2-v14-device-profile/translate6.alpha"
            ),
            28,
            36,
            6,
        ),
        (
            2.,
            0.,
            include_bytes!("../../../../../conformance/pencil2-v14-device-profile/scale2.alpha"),
            65,
            68,
            2,
        ),
    ];
    assert_eq!(NATIVE_STREAMS[9408..14112].len(), 84 * 56);
    for (scale, tx, native, expected_count, expected_sum, expected_max) in cases {
        assert_eq!(native.len(), 62 * 22);
        let image = render(scale, tx);
        let mut count = 0;
        let mut sum = 0u32;
        let mut max = 0u8;
        for y in 170..192 {
            for x in 0..62 {
                let error = image[y * 192 + x].abs_diff(native[(y - 170) * 62 + x]);
                count += usize::from(error != 0);
                sum += u32::from(error);
                max = max.max(error);
            }
        }
        assert_eq!(
            (count, sum, max),
            (expected_count, expected_sum, expected_max)
        );
        assert!(
            image
                .iter()
                .enumerate()
                .all(|(i, &a)| a == 0 || (i % 192 < 62 && i / 192 >= 170))
        );
    }
}
