//! Fixed output-grid approximation. This is not a scalable material.
use super::Record;
#[cfg(test)]
use std::fmt::Write;

pub(super) struct Grid<'a> {
    pub width: u32,
    pub height: u32,
    /// Axis-aligned clip transform: clip = coordinate * scale + translation.
    pub clip_scale: [f32; 2],
    pub clip_translation: [f32; 2],
    pub background_scale: [f32; 2],
    pub rgba: [f32; 4],
    pub point: &'a [u8],
    pub background: &'a [u8],
    pub max_samples: u64,
}

fn plane(p: [[f32; 2]; 3], a: [f32; 3]) -> [f32; 3] {
    let dx01 = p[0][0] - p[1][0];
    let dy01 = p[0][1] - p[1][1];
    let dx20 = p[2][0] - p[0][0];
    let dy20 = p[2][1] - p[0][1];
    let inv = 1.0 / (dx01 * dy20 - dy01 * dx20);
    let da01 = a[0] - a[1];
    let da20 = a[2] - a[0];
    let dx = da01 * (dy20 * inv) - da20 * (dy01 * inv);
    let dy = da20 * (dx01 * inv) - da01 * (dx20 * inv);
    [a[0] - (dx * (p[0][0] - 0.5) + dy * (p[0][1] - 0.5)), dx, dy]
}

fn viewport_coordinate(clip: f32, extent: u32) -> f32 {
    let half_extent = extent as f32 * 0.5;
    clip.mul_add(half_extent, half_extent)
}

#[test]
fn viewport_matches_captured_fused_operation() {
    let clip = f32::from_bits(0xbeb8_d605);
    assert_eq!(viewport_coordinate(clip, 160).to_bits(), 0x424c_7a3d);
    assert_eq!(((clip + 1.0) * 0.5 * 160.0).to_bits(), 0x424c_7a3e);
}

fn texture_index(value: f32) -> Result<usize, &'static str> {
    let value = value.floor() as f64;
    if !value.is_finite() || !(-9223372036854775808.0..9223372036854775808.0).contains(&value) {
        return Err("invalid texture coordinate");
    }
    Ok((value as i64).rem_euclid(150) as usize)
}

fn repeat_texture_index(value: f32) -> Result<usize, &'static str> {
    // Keep the existing range bound before reducing the normalized coordinate.
    texture_index(value * 150.)?;
    let wrapped = value.rem_euclid(1.);
    if wrapped == 1. {
        return Ok(149);
    }
    texture_index(wrapped * 150.)
}

fn value(p: [f32; 3], x: u32, y: u32) -> f32 {
    p[2].mul_add(y as f32, p[1].mul_add(x as f32, p[0]))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridFragment {
    pub x: u32,
    pub y: u32,
    pub rgb: [u8; 3],
    pub alpha: f32,
}

pub(super) fn sample(records: &[Record], g: &Grid<'_>) -> Result<(Vec<GridFragment>, u64), &'static str> {
    if g.width == 0
        || g.height == 0
        || g.width > 4096
        || g.height > 4096
        || g.point.len() != 300
        || g.background.len() != 22500
        || g.clip_scale.iter().any(|v| !v.is_finite() || *v <= 0.0)
        || g.clip_translation
            .iter()
            .chain(g.background_scale.iter())
            .any(|v| !v.is_finite())
        || g.rgba
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
        return Err("invalid grid context");
    }
    if records.len() > 65_536 {
        return Err("grid record limit");
    }
    let rgb = g.rgba[..3]
        .iter()
        .map(|x| (x * 255.0).round() as u8)
        .collect::<Vec<_>>();
    let mut fragments = Vec::new();
    let mut work = 0u64;
    for r in records {
        let [x, y, rad, sel, bx, by] = r.lanes;
        if r.lanes.iter().any(|v| !v.is_finite()) || rad <= 0.0 {
            return Err("invalid record");
        }
        let mut coords = [[0.; 2]; 4];
        let mut uv = [[0.; 4]; 4];
        for (i, (sx, sy, pu, pv)) in [
            (-1., -1., 0., 1.),
            (-1., 1., 0., 0.),
            (1., -1., 1., 1.),
            (1., 1., 1., 0.),
        ]
        .into_iter()
        .enumerate()
        {
            for (axis, (coord, off, sg)) in [(x, bx, sx), (y, by, sy)].into_iter().enumerate() {
                let v = coord + sg * rad;
                let b = v + off;
                let clip = g.clip_scale[axis] * v + g.clip_translation[axis];
                let bc = g.clip_scale[axis] * b + g.clip_translation[axis];
                coords[i][axis] = viewport_coordinate(clip, [g.width, g.height][axis]);
                uv[i][axis] = (bc + 1.) * 0.5;
            }
            uv[i][2] = pu;
            uv[i][3] = (1.0f32 / 3.0) * (sel + pv);
        }
        let [x0, y0] = coords[0];
        let x1 = coords[2][0];
        let y1 = coords[1][1];
        if !coords
            .iter()
            .flatten()
            .chain(uv.iter().flatten())
            .all(|v| v.is_finite())
            || x1 <= x0
            || y1 <= y0
        {
            return Err("invalid projected record");
        }
        let left = ((x0 - 0.5).ceil().max(0.) as u32).min(g.width);
        let right = ((x1 - 0.5).ceil().max(0.) as u32).min(g.width);
        let bottom = ((y0 - 0.5).ceil().max(0.) as u32).min(g.height);
        let top = ((y1 - 0.5).ceil().max(0.) as u32).min(g.height);
        work = work
            .checked_add(u64::from(right - left) * u64::from(top - bottom))
            .ok_or("grid work limit")?;
        if work > g.max_samples.min(100_000) {
            return Err("grid work limit");
        }
        if left == right || bottom == top {
            continue;
        }
        let mut planes = [[[0.; 3]; 4]; 2];
        for (t, ids) in [[0, 1, 2], [2, 1, 3]].into_iter().enumerate() {
            for a in 0..4 {
                planes[t][a] = plane(ids.map(|i| coords[i]), ids.map(|i| uv[i][a]));
            }
        }
        if !planes.iter().flatten().flatten().all(|v| v.is_finite()) {
            return Err("invalid interpolation plane");
        }
        for py in bottom..top {
            for px in left..right {
                let a = (px as f64 + 0.5 - x0 as f64) / (x1 - x0) as f64;
                let b = (py as f64 + 0.5 - y0 as f64) / (y1 - y0) as f64;
                let p = planes[usize::from(a + b > 1.)];
                let [ux, uy, pu, pv] = p.map(|v| value(v, px, py));
                if ![
                    ux,
                    uy,
                    pu,
                    pv,
                    ux * g.background_scale[0] * 150.,
                    uy * g.background_scale[1] * 150.,
                ]
                .iter()
                .all(|v| v.is_finite())
                {
                    return Err("invalid interpolated sample");
                }
                let tx = repeat_texture_index(ux * g.background_scale[0])?;
                let ty = repeat_texture_index(uy * g.background_scale[1])?;
                let ix = ((pu * 10.).floor() as i32).clamp(0, 9) as usize;
                let iy = ((pv * 30.).floor() as i32).clamp(0, 29) as usize;
                let alpha = (g.rgba[3]
                    * (g.point[iy * 10 + ix] as f32 / 255.)
                    * (g.background[ty * 150 + tx] as f32 / 255.)
                    * 5.)
                    .min(1.);
                if alpha > 0. {
                    fragments.push(GridFragment { x: px, y: g.height - 1 - py, rgb: [rgb[0], rgb[1], rgb[2]], alpha });
                }
            }
        }
    }
    Ok((fragments, work))
}

#[cfg(test)]
pub(super) fn fragments(records: &[Record], g: &Grid<'_>) -> Result<String, &'static str> {
    let (fragments, _) = sample(records, g)?;
    let mut svg = String::new();
    for fragment in fragments {
        let GridFragment { x, y, rgb: [r, g, b], alpha } = fragment;
        write!(svg, "<rect x=\"{x}\" y=\"{y}\" width=\"1\" height=\"1\" fill=\"#{r:02x}{g:02x}{b:02x}\" fill-opacity=\"{alpha:.9}\"/>\n").unwrap();
    }
    Ok(svg)
}

#[test]
fn explicit_context_and_work_budget() {
    let point = [255; 300];
    let background = [255; 22500];
    let mut g = Grid {
        width: 8,
        height: 8,
        clip_scale: [0.25; 2],
        clip_translation: [-1.; 2],
        background_scale: [1.; 2],
        rgba: [1., 0., 0., 0.1],
        point: &point,
        background: &background,
        max_samples: 100,
    };
    let records = [Record {
        lanes: [4., 4., 1., 0., 0., 0.],
    }];
    let svg = fragments(&records, &g).unwrap();
    assert_eq!(svg.matches("<rect").count(), 4);
    assert!(!svg.contains("<svg"));
    g.max_samples = 3;
    assert_eq!(fragments(&records, &g), Err("grid work limit"));
}

#[test]
fn pathological_inputs_are_bounded() {
    let point = [255; 300];
    let background = [255; 22500];
    let mut g = Grid {
        width: 2048,
        height: 2048,
        clip_scale: [1.; 2],
        clip_translation: [0.; 2],
        background_scale: [1.; 2],
        rgba: [1.; 4],
        point: &point,
        background: &background,
        max_samples: u64::MAX,
    };
    let large = Record {
        lanes: [0., 0., 1., 0., 0., 0.],
    };
    assert_eq!(fragments(&[large], &g), Err("grid work limit"));
    let overflow = Record {
        lanes: [f32::MAX, 0., f32::MAX, 0., 0., 0.],
    };
    assert_eq!(fragments(&[overflow], &g), Err("invalid projected record"));
    let outside = Record {
        lanes: [20., 20., 1., 0., 0., 0.],
    };
    assert_eq!(
        fragments(&vec![outside; 65_537], &g),
        Err("grid record limit")
    );
    assert_eq!(fragments(&[outside], &g).unwrap(), "");
    g.width = 8;
    g.height = 8;
    g.clip_scale = [0.25; 2];
    g.clip_translation = [-1.; 2];
    let covered = Record {
        lanes: [4., 4., 1., 0., 0., 0.],
    };
    g.background_scale = [1e20; 2];
    assert_eq!(fragments(&[covered], &g), Err("invalid texture coordinate"));
    g.background_scale = [f32::MAX; 2];
    assert_eq!(
        fragments(&[covered], &g),
        Err("invalid interpolated sample")
    );
    g.background_scale = [1.; 2];
    g.clip_scale = [f32::MIN_POSITIVE; 2];
    assert_eq!(fragments(&[large], &g), Err("invalid projected record"));
}

#[test]
fn normalized_repeat_precedes_texel_scaling() {
    assert_eq!(repeat_texture_index(f32::from_bits(0x3f8369d0)), Ok(3));
    assert_eq!(repeat_texture_index(f32::from_bits(0x3f4962fc)), Ok(117));
    assert_eq!(repeat_texture_index(-0.25), Ok(112));
    assert_eq!(repeat_texture_index(-f32::MIN_POSITIVE), Ok(149));
    assert_eq!(repeat_texture_index(0.), Ok(0));
    assert_eq!(repeat_texture_index(2.25), Ok(37));
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, -f32::MAX] {
        assert_eq!(repeat_texture_index(value), Err("invalid texture coordinate"));
    }
}
