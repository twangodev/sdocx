mod clip;
pub(crate) mod pdf;
use std::fmt::Write as _;
const TEX: &[u8] = include_bytes!("cap-textures-rgba8.bin");
fn path(points: &[Vec<f64>]) -> Option<String> {
    if points.len() < 3 {
        return None;
    }
    let mut s = String::new();
    for (i, p) in points.iter().enumerate() {
        if p.len() < 2
            || p[0].abs() > 1e6
            || p[1].abs() > 1e6
            || !p[0].is_finite()
            || !p[1].is_finite()
        {
            return None;
        }
        writeln!(
            s,
            "{:.9} {:.9} {}",
            p[0],
            p[1],
            if i == 0 { "m" } else { "l" }
        )
        .ok()?;
    }
    s.push_str("h\n");
    Some(s)
}
fn area(t: &[Vec<f64>]) -> f64 {
    (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0])
}
fn gradient(t: &[Vec<f64>], z: [f64; 3]) -> Option<[f32; 4]> {
    let d = area(t);
    if d == 0. || !d.is_finite() {
        return None;
    }
    let a = ((z[1] - z[0]) * (t[2][1] - t[0][1]) - (z[2] - z[0]) * (t[1][1] - t[0][1])) / d;
    let b = ((t[1][0] - t[0][0]) * (z[2] - z[0]) - (t[2][0] - t[0][0]) * (z[1] - z[0])) / d;
    let n = a * a + b * b;
    if n < 1e-20 || !n.is_finite() {
        return None;
    }
    let g = [
        t[0][0] - z[0] * a / n,
        t[0][1] - z[0] * b / n,
        t[0][0] + (1. - z[0]) * a / n,
        t[0][1] + (1. - z[0]) * b / n,
    ];
    if g.iter().any(|x| !x.is_finite() || x.abs() > 1e6) {
        return None;
    }
    Some(g.map(|x| x as f32))
}
fn cap_path(cap: &[Vec<f64>], texture: usize, reverse: bool) -> Option<String> {
    if cap.len() < 3 || texture >= 2 {
        return None;
    }
    let mut s = String::new();
    let p = &cap[0];
    let b = &cap[1];
    let c = &cap[2];
    let pt = |u: f64, v: f64| {
        vec![
            p[0] + u * (b[0] - p[0]) + v * (c[0] - p[0]),
            p[1] + u * (b[1] - p[1]) + v * (c[1] - p[1]),
        ]
    };
    for y in 0..72 {
        let mut x = 0;
        while x < 54 {
            if TEX[texture * 54 * 72 * 4 + (y * 54 + x) * 4 + 3] == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < 54 && TEX[texture * 54 * 72 * 4 + (y * 54 + x) * 4 + 3] != 0 {
                x += 1
            }
            let (mut u0, mut u1) = (start as f64 / 54., x as f64 / 54.);
            let (mut v0, mut v1) = (y as f64 / 72., (y + 1) as f64 / 72.);
            if reverse {
                (u0, u1) = (1. - u1, 1. - u0);
                (v0, v1) = (1. - v1, 1. - v0)
            }
            s.push_str(&path(&[pt(u0, v0), pt(u1, v0), pt(u1, v1), pt(u0, v1)])?);
            if s.len() > 400_000 {
                return None;
            }
        }
    }
    Some(s)
}
fn run(
    m: &crate::ink::legacy_marker_mesh::Mesh,
    paint: u32,
    grid: f64,
    shapes: &[clip::VisiblePolygon],
    page: [f32; 2],
) -> Option<(pdf::Plan, [f32; 4])> {
    if m.body.len() < 3
        || m.caps.iter().any(|cap| cap.len() != 6)
        || !matches!(grid, 10_000. | 100_000. | 300_000.)
        || page.iter().any(|v| !v.is_finite() || *v <= 0.0)
    {
        return None;
    }
    let mesh_vertices = m
        .body
        .len()
        .checked_add(
            m.caps
                .iter()
                .try_fold(0usize, |n, v| n.checked_add(v.len()))?,
        )?
        .checked_add(
            m.fringe
                .iter()
                .try_fold(0usize, |n, v| n.checked_add(v.len()))?,
        )?;
    if mesh_vertices > 1024
        || m.body
            .iter()
            .flatten()
            .chain(m.caps.iter().flatten().flatten())
            .chain(m.fringe.iter().flatten().flatten())
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
    {
        return None;
    }
    let output_vertices = shapes
        .iter()
        .flatten()
        .flatten()
        .try_fold(0usize, |n, contour| n.checked_add(contour.len()))?;
    if output_vertices > 12_000 {
        return None;
    }
    let body = m
        .body
        .iter()
        .map(|v| v.iter().map(|x| f64::from(*x)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let cap0 = m.caps[0]
        .iter()
        .map(|v| v.iter().map(|x| f64::from(*x)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let cap1 = m.caps[1]
        .iter()
        .map(|v| v.iter().map(|x| f64::from(*x)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let fringe = m.fringe.each_ref().map(|f| {
        f.iter()
            .map(|v| v.iter().map(|x| f64::from(*x)).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    });
    let all = body
        .iter()
        .chain(cap0.iter())
        .chain(cap1.iter())
        .chain(fringe.iter().flatten());
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for v in all {
        bounds[0] = bounds[0].min(v[0]);
        bounds[1] = bounds[1].min(v[1]);
        bounds[2] = bounds[2].max(v[0]);
        bounds[3] = bounds[3].max(v[1]);
    }
    bounds = [
        bounds[0].floor() - 1.,
        bounds[1].floor() - 1.,
        bounds[2].ceil() + 1.,
        bounds[3].ceil() + 1.,
    ];
    let color = [
        ((paint >> 16) & 255) as f32 / 255.,
        ((paint >> 8) & 255) as f32 / 255.,
        (paint & 255) as f32 / 255.,
    ];
    let alpha = (paint >> 24) as f32 / 255.;
    let mut body_path = String::new();
    for tri in body.windows(3) {
        let a = area(tri);
        if a.abs() <= 1e-8 {
            continue;
        }
        let mut t = tri.to_vec();
        if a < 0. {
            t.reverse()
        }
        body_path.push_str(&path(&t)?)
    }
    if body_path.is_empty() {
        return None;
    }
    let mut draws = vec![pdf::Draw {
        path: body_path.clone(),
        color,
        alpha,
        gradient: None,
        owner: None,
        even_odd: false,
    }];
    let cap_grad = |c: &[Vec<f64>]| {
        gradient(
            &c[..3],
            [alpha as f64, alpha as f64 + 54. / 255., alpha as f64],
        )
    };
    draws.push(pdf::Draw {
        path: cap_path(&cap0, 0, true)?,
        color,
        alpha: 1.,
        gradient: Some(cap_grad(&cap0)?),
        owner: Some(body_path),
        even_odd: false,
    });
    let mut visible = shapes.iter();
    let origin = &body[0];
    for side in fringe {
        for tri in side.windows(3) {
            let coords = tri
                .iter()
                .map(|p| {
                    [
                        ((p[0] - origin[0]) * grid).round() as i128,
                        ((p[1] - origin[1]) * grid).round() as i128,
                    ]
                })
                .collect::<Vec<_>>();
            if (coords[1][0] - coords[0][0]) * (coords[2][1] - coords[0][1])
                == (coords[1][1] - coords[0][1]) * (coords[2][0] - coords[0][0])
            {
                continue;
            }
            let shape = visible.next()?;
            let mut d = String::new();
            for polygon in shape {
                for contour in polygon {
                    let p = contour.iter().map(|v| vec![v[0], v[1]]).collect::<Vec<_>>();
                    d.push_str(&path(&p)?);
                }
            }
            if d.is_empty() {
                continue;
            }
            let a = alpha as f64;
            let g = gradient(
                tri,
                [
                    a * (1. - tri[0][2]),
                    a * (1. - tri[1][2]),
                    a * (1. - tri[2][2]),
                ],
            )?;
            draws.push(pdf::Draw {
                path: d,
                color,
                alpha: 1.,
                gradient: Some(g),
                owner: None,
                even_odd: true,
            });
            if draws.len() > 512 {
                return None;
            }
        }
    }
    if visible.next().is_some() {
        return None;
    }
    draws.push(pdf::Draw {
        path: cap_path(&cap1, 1, false)?,
        color,
        alpha: 1.,
        gradient: Some(cap_grad(&cap1)?),
        owner: None,
        even_odd: false,
    });
    if draws
        .iter()
        .try_fold(0usize, |n, d| {
            n.checked_add(d.path.len())?
                .checked_add(d.owner.as_ref().map_or(0, String::len))
        })
        .is_none_or(|n| n > 400_000)
    {
        return None;
    }
    Some((
        pdf::Plan {
            width: page[0],
            height: page[1],
            draws,
        },
        bounds.map(|x| x as f32),
    ))
}
#[derive(Default)]
pub(crate) struct SourcePageBudget {
    pub(super) points: usize,
    mesh_vertices: usize,
    clip_ops: usize,
    visible_vertices: usize,
    svg_bytes: usize,
    pdf: pdf::PageBudget,
}
pub(super) struct SourceStroke {
    pub(super) points: Vec<[f32; 2]>,
    pub(super) size: f32,
    pub(super) screen: [u32; 2],
    pub(super) argb: u32,
    pub(super) scale: f32,
    pub(super) page: [f32; 2],
}
pub(super) struct PreparedStroke {
    pub(super) plan: pdf::Plan,
    pub(super) form: pdf::Prepared,
    pub(super) bounds: [f32; 4],
}
pub(super) fn execute(
    stroke: &SourceStroke,
    budget: &mut SourcePageBudget,
) -> Option<PreparedStroke> {
    let points = &stroke.points;
    if points.len() > 256 {
        return None;
    }
    budget.points = budget.points.checked_add(points.len())?;
    if budget.points > 1024 {
        return None;
    }
    let mut m =
        crate::ink::legacy_marker_mesh::generate_bounded_mesh(points, stroke.size, stroke.screen)?;
    let mesh_vertices = m
        .body
        .len()
        .checked_add(m.fringe.iter().map(Vec::len).sum::<usize>())?
        .checked_add(m.caps.iter().map(Vec::len).sum::<usize>())?;
    budget.mesh_vertices = budget.mesh_vertices.checked_add(mesh_vertices)?;
    if budget.mesh_vertices > 16_384 {
        return None;
    }
    let scale = stroke.scale;
    if !scale.is_finite() || scale <= 0. || scale > 4. {
        return None;
    }
    for p in &mut m.body {
        p[0] *= scale;
        p[1] *= scale
    }
    for p in m.caps.iter_mut().flatten() {
        p[0] *= scale;
        p[1] *= scale
    }
    for p in m.fringe.iter_mut().flatten() {
        p[0] *= scale;
        p[1] *= scale
    }
    let paint = stroke.argb;
    let mut collapse = false;
    let limit = budget.clip_ops.saturating_add(200_000).min(500_000);
    let (mut grid, mut shapes) = (
        10000.,
        clip::run(&m, &mut budget.clip_ops, limit, &mut collapse, 10000.),
    );
    if shapes.is_none() && collapse && budget.clip_ops < limit {
        collapse = false;
        grid = 100000.;
        shapes = clip::run(&m, &mut budget.clip_ops, limit, &mut collapse, grid)
    }
    if shapes.is_none() && collapse && budget.clip_ops < limit {
        collapse = false;
        grid = 300_000.;
        shapes = clip::run(&m, &mut budget.clip_ops, limit, &mut collapse, grid);
    }
    let (shape, vertices) = shapes?;
    let next_vertices = budget.visible_vertices.checked_add(vertices)?;
    if next_vertices > 30_000 {
        return None;
    }
    let (plan, bounds) = run(&m, paint, grid, &shape, stroke.page)?;
    // Each finite f32 coordinate has a bounded decimal representation. Reserve
    // a conservative 128 bytes per path command and 1024 per draw for nodes,
    // attributes and gradient/mask definitions before publishing the scene.
    let svg_cost = plan.draws.iter().try_fold(0usize, |n, d| {
        let commands = d
            .path
            .lines()
            .count()
            .checked_add(d.owner.as_ref().map_or(0, |p| p.lines().count()))?;
        n.checked_add(commands.checked_mul(128)?)?.checked_add(1024)
    })?;
    let next_svg = budget.svg_bytes.checked_add(svg_cost)?;
    if next_svg > 1_000_000 {
        return None;
    }
    let mut pdf_budget = budget.pdf;
    let form = match pdf::prepare(&plan, bounds, &mut pdf_budget) {
        Some(form) => form,
        None => {
            budget.pdf.charge_attempts_from(pdf_budget);
            return None;
        }
    };
    budget.pdf = pdf_budget;
    budget.visible_vertices = next_vertices;
    budget.svg_bytes = next_svg;

    Some(PreparedStroke { plan, form, bounds })
}

fn scene_data(s: &str) -> Option<super::vector::Data> {
    let mut data = super::vector::Data::new();
    for line in s.lines() {
        let p = line.split_whitespace().collect::<Vec<_>>();
        data = match p.as_slice() {
            ["h"] => data.close(),
            [x, y, "m"] => data.move_to((x.parse::<f64>().ok()?, y.parse::<f64>().ok()?)),
            [x, y, "l"] => data.line_to((x.parse::<f64>().ok()?, y.parse::<f64>().ok()?)),
            _ => return None,
        };
    }
    Some(data)
}

pub(super) fn emit(scene: &mut super::vector::Scene, prepared: PreparedStroke) -> bool {
    use super::vector::{
        ColorValue, Data, Definitions, Gradient, Group, LinearGradient, Mask, Paint, Path,
        Rectangle, Stop, Styled, SvgMask,
    };
    let compiled = prepared
        .plan
        .draws
        .iter()
        .map(|d| {
            Some((
                scene_data(&d.path)?,
                match d.owner.as_deref() {
                    Some(v) => Some(scene_data(v)?),
                    None => None,
                },
            ))
        })
        .collect::<Option<Vec<(Data, Option<Data>)>>>();
    let Some(compiled) = compiled else {
        return false;
    };
    let [x, y, x2, y2] = prepared.bounds;
    let group = Group::new();
    let group = if scene.retains_text() {
        let bounds = crate::BoundingBox {
            x_min: f64::from(x),
            y_min: f64::from(y),
            x_max: f64::from(x2),
            y_max: f64::from(y2),
        };
        let id = scene
            .native_ink()
            .register_legacy_marker(prepared.form, bounds);
        group.native_ink_id(id)
    } else {
        group
    };
    scene.scope(group, |scene| {
        for (draw, (path, owner)) in prepared.plan.draws.into_iter().zip(compiled) {
            let Some(color) = ColorValue::rgb_components(
                f64::from(draw.color[0]),
                f64::from(draw.color[1]),
                f64::from(draw.color[2]),
            ) else {
                continue;
            };
            let paint = if let Some(g) = draw.gradient {
                let id = scene.definition::<Gradient>();
                scene.push(
                    Definitions::new().add(
                        LinearGradient::new(&id)
                            .x1(g[0])
                            .y1(g[1])
                            .x2(g[2])
                            .y2(g[3])
                            .add(Stop::new(0, color).opacity(0))
                            .add(Stop::new(1, color).opacity(1)),
                    ),
                );
                Paint::Gradient(id)
            } else {
                Paint::Solid(color)
            };
            let mut node = Path::new().data(path).fill(paint).fill_opacity(draw.alpha);
            if draw.even_odd {
                node = node.even_odd_fill()
            }
            if let Some(owner) = owner {
                let id = scene.definition::<Mask>();
                scene.push(
                    Definitions::new().add(
                        SvgMask::luminance(&id)
                            .x(x)
                            .y(y)
                            .width(x2 - x)
                            .height(y2 - y)
                            .add(
                                Rectangle::new()
                                    .x(x)
                                    .y(y)
                                    .width(x2 - x)
                                    .height(y2 - y)
                                    .fill(Paint::Solid(ColorValue::gray(1.).expect("valid white"))),
                            )
                            .add(Path::new().data(owner).fill(Paint::BLACK)),
                    ),
                );
                node = node.masked(&id);
            }
            scene.push(node);
        }
    });
    true
}

pub(super) fn render(
    scene: &mut super::vector::Scene,
    stroke: &crate::Stroke,
    theme: super::theme::RenderTheme,
    page: [u32; 2],
    profile: super::LegacyMarkerCenterlineProfile,
    budget: &mut SourcePageBudget,
) -> bool {
    if page.contains(&0)
        || profile.screen_width == 0
        || profile.screen_height == 0
        || profile.screen_width > i32::MAX as u32
        || profile.screen_height > i32::MAX as u32
        || !(3..=256).contains(&stroke.points.len())
    {
        return false;
    }
    let Some(argb) = crate::ink::legacy_marker_profile_argb(stroke) else {
        return false;
    };
    let Some(rendering) = stroke.rendering.as_ref() else {
        return false;
    };
    if rendering
        .metadata
        .as_ref()
        .and_then(|m| m.rotation_degrees)
        .is_some_and(|degrees| degrees != 0.)
    {
        return false;
    }
    let Some(size) = rendering.style.pen_size else {
        return false;
    };
    if !size.is_finite() || !(2.0..=800.0).contains(&size) {
        return false;
    }
    let size = size.trunc();
    let color = theme.foreground(stroke.color.or(Some(super::argb_rgb(argb))));
    let Some(rgb) = color
        .strip_prefix('#')
        .and_then(|s| u32::from_str_radix(s, 16).ok())
    else {
        return false;
    };
    let source = SourceStroke {
        points: stroke
            .points
            .iter()
            .map(|p| [p.x as f32, p.y as f32])
            .collect(),
        size,
        screen: [profile.screen_width, profile.screen_height],
        argb: (argb & 0xff00_0000) | rgb,
        scale: 1.,
        page: [page[0] as f32, page[1] as f32],
    };
    let Some(prepared) = execute(&source, budget) else {
        return false;
    };
    emit(scene, prepared)
}

#[cfg(test)]
mod tests;
