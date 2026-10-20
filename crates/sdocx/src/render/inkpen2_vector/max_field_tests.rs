use super::super::render;
use super::{MaxAtlas, Region};
use crate::render::vector::{self, Scene, Styled, Svg};
use crate::{BoundingBox, Color, PreparedInkPen2V9};

fn ink(records: Vec<[f32; 9]>, bounds: BoundingBox) -> PreparedInkPen2V9 {
    let count = records.len();
    PreparedInkPen2V9 {
        per_stamp_rgb: true,
        enhanced_antialias: true,
        records,
        sample_ends: vec![0, count],
        color: Color {
            r: 20,
            g: 40,
            b: 60,
        },
        opacity: 1.0,
        bounds,
    }
}

fn region_contains(outer: Region, inner: Region) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn render_mask(ink: &PreparedInkPen2V9, width: u32, height: u32) -> resvg::tiny_skia::Pixmap {
    let atlas = MaxAtlas::new(ink).expect("valid atlas inputs");
    let mut scene = Scene::new(Svg::new().width(width).height(height));
    let mask = scene.definition::<vector::Mask>();
    scene.scope(vector::Definitions::new(), |svg| {
        atlas.emit(svg, ink, &mask, false);
    });
    scene.scope(vector::Group::new().masked(&mask), |svg| {
        svg.push(
            vector::Rectangle::new()
                .x(0)
                .y(0)
                .width(f64::from(width))
                .height(f64::from(height))
                .fill(vector::Paint::Solid(vector::ColorValue::WHITE)),
        );
    });
    let tree = resvg::usvg::Tree::from_str(&scene.finish(), &Default::default())
        .expect("atlas SVG parses");
    let mut pixels = resvg::tiny_skia::Pixmap::new(width, height).expect("pixel buffer");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    pixels
}

fn pixel_alpha(pixels: &resvg::tiny_skia::Pixmap, x: u32, y: u32) -> u8 {
    pixels.pixel(x, y).expect("pixel lies in frame").alpha()
}

#[test]
fn planner_preserves_fractional_phases_and_packs_disjoint_backplates() {
    // Five records force an odd, three-column atlas. Negative fractional
    // coordinates exercise floor-based phase preservation on both axes.
    let ink = ink(
        vec![
            [-10.25, -1.5, 3.75, 4.5, 0.3, 0.8, 1., 0., 0.],
            [10.5, 1.25, 3.75, 4.5, 0.3, 0.6, 0., 0., 1.],
            [30.25, 4.5, 3.75, 4.5, 0.3, 0.7, 1., 0., 0.],
            [-10.25, 12.5, 3.75, 4.5, 0.3, 0.6, 0., 1., 0.],
            [10.5, 15.5, 3.75, 4.5, 0.3, 0.9, 1., 0., 1.],
        ],
        BoundingBox {
            x_min: -14.0,
            y_min: -6.0,
            x_max: 34.0,
            y_max: 20.0,
        },
    );
    let atlas = MaxAtlas::new(&ink).unwrap();
    assert_eq!(atlas.cells.len(), 5);

    for (cell, record) in atlas.cells.iter().zip(&ink.records) {
        for (axis, &coordinate) in record.iter().take(2).enumerate() {
            let source = f64::from(coordinate);
            let fraction = source - source.floor();
            assert_eq!(cell.center[axis].rem_euclid(1.0), fraction);
            assert_eq!(cell.offset[axis].fract(), 0.0);
        }
        assert!(region_contains(atlas.filter, cell.backplate));
        assert!(region_contains(atlas.filter, cell.crop));
    }
    assert_eq!(atlas.cells[0].center[0].rem_euclid(1.0), 0.75);
    assert_eq!(atlas.cells[0].center[1].rem_euclid(1.0), 0.5);

    for (index, left) in atlas.cells.iter().enumerate() {
        for right in atlas.cells.iter().skip(index + 1) {
            let a = left.backplate;
            let b = right.backplate;
            assert!(
                a.x + a.width <= b.x
                    || b.x + b.width <= a.x
                    || a.y + a.height <= b.y
                    || b.y + b.height <= a.y,
                "packed backplates overlap"
            );
        }
    }
}

#[test]
fn atlas_mask_crops_distant_asymmetric_stamps_without_phantom_cells() {
    let ink = ink(
        vec![
            [8.5, 9.5, 3.75, 4.5, 0.3, 0.8, 1., 0., 0.],
            [40.5, 12.5, 4.5, 4.5, 0.3, 0.6, 0., 0., 1.],
        ],
        BoundingBox {
            x_min: 4.75,
            y_min: 5.0,
            x_max: 45.0,
            y_max: 17.0,
        },
    );
    let pixels = render_mask(&ink, 64, 32);
    assert!((i16::from(pixel_alpha(&pixels, 8, 9)) - 204).abs() <= 1);
    assert!((i16::from(pixel_alpha(&pixels, 40, 12)) - 153).abs() <= 1);
    assert_eq!(pixel_alpha(&pixels, 27, 12), 0, "phantom atlas gap");
    assert_eq!(pixel_alpha(&pixels, 3, 9), 0, "outside first ellipse");
    assert_eq!(pixel_alpha(&pixels, 46, 12), 0, "outside second ellipse");
}

#[test]
fn atlas_mask_keeps_max_coverage_for_coincident_large_stamps() {
    let mut ink = ink(
        vec![
            [32.5, 32.5, 8., 8., 0.3, 0.8, 1., 0., 0.],
            [32.5, 32.5, 8., 8., 0.3, 0.6, 0., 0., 1.],
        ],
        BoundingBox {
            x_min: 24.5,
            y_min: 24.5,
            x_max: 40.5,
            y_max: 40.5,
        },
    );
    ink.opacity = 128.0 / 255.0;
    for _ in 0..2 {
        let pixels = render_mask(&ink, 64, 64);
        assert!((i16::from(pixel_alpha(&pixels, 32, 32)) - 204).abs() <= 1);
        ink.records.reverse();
    }
}

#[test]
fn atlas_coverage_is_not_quantized_before_the_radial_falloff() {
    let ink = ink(
        vec![[32.5, 32.5, 8., 8., 0.1, 0.0021, 1., 0., 0.]],
        BoundingBox {
            x_min: 24.5,
            y_min: 24.5,
            x_max: 40.5,
            y_max: 40.5,
        },
    );
    let pixels = render_mask(&ink, 64, 64);
    // At the center .0021*255 rounds to one. Four units away, the
    // radial multiplier is .625, so native R8 coverage rounds to zero.
    // Quantizing a gray stop first would interpolate 1*.625 and emit one.
    assert_eq!(pixel_alpha(&pixels, 32, 32), 1);
    assert_eq!(pixel_alpha(&pixels, 36, 32), 0);
}

#[test]
fn serialized_rgb_replay_prefix_removes_only_later_stamp_channels() {
    let ink = ink(
        vec![
            [8.5, 9.5, 3.75, 4.5, 0.3, 0.8, 1., 0., 0.],
            [40.5, 12.5, 4.5, 4.5, 0.3, 0.6, 0., 0., 1.],
        ],
        BoundingBox {
            x_min: 4.75,
            y_min: 5.0,
            x_max: 45.0,
            y_max: 17.0,
        },
    );
    let mut scene = Scene::new(Svg::new().width(64).height(32));
    render(&mut scene, &ink, Some(3));
    let complete = scene.finish();
    let document = roxmltree::Document::parse(&complete).expect("serialized SVG parses");
    let later_parts = document
        .descendants()
        .filter(|node| node.attribute("data-replay-part") == Some("2"))
        .map(|node| node.range())
        .collect::<Vec<_>>();
    assert_eq!(
        later_parts.len(),
        2,
        "later mask and color stamps are tagged"
    );
    let mut prefix = complete;
    for range in later_parts.into_iter().rev() {
        prefix.replace_range(range, "");
    }

    let tree = resvg::usvg::Tree::from_str(&prefix, &Default::default())
        .expect("replay prefix SVG parses");
    let mut pixels = resvg::tiny_skia::Pixmap::new(64, 32).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    assert!(pixel_alpha(&pixels, 8, 9) > 150, "first stamp survives");
    assert_eq!(pixel_alpha(&pixels, 40, 12), 0, "second stamp is removed");
    assert_eq!(
        pixel_alpha(&pixels, 23, 12),
        0,
        "removed input cannot shift its predecessor"
    );
    assert_eq!(
        pixel_alpha(&pixels, 27, 12),
        0,
        "no phantom MAX field remains"
    );
}

#[test]
fn atlas_planner_rejects_invalid_inputs_and_resource_limits() {
    let valid_record = [0., 0., 1., 1., 0.25, 0.8, 1., 0., 0.];
    let valid_bounds = BoundingBox {
        x_min: -1.,
        y_min: -1.,
        x_max: 1.,
        y_max: 1.,
    };
    assert!(MaxAtlas::new(&ink(vec![valid_record], valid_bounds)).is_ok());

    let bad_records = [
        [f32::NAN, 0., 1., 1., 0.25, 0.8, 1., 0., 0.],
        [0., 0., f32::INFINITY, 1., 0.25, 0.8, 1., 0., 0.],
        [0., 0., 0., 1., 0.25, 0.8, 1., 0., 0.],
        [0., 0., 1., 1., 0.5, 0.8, 1., 0., 0.],
        [0., 0., 1., 1., -0.01, 0.8, 1., 0., 0.],
        [0., 0., 1., 1., 0.25, 1.01, 1., 0., 0.],
        [0., 0., 1., 1., 0.25, f32::NAN, 1., 0., 0.],
        [1.1, 0., 1., 1., 0.25, 0.8, 1., 0., 0.],
    ];
    for record in bad_records {
        assert!(MaxAtlas::new(&ink(vec![record], valid_bounds)).is_err());
    }

    for bounds in [
        BoundingBox {
            x_min: f64::NAN,
            ..valid_bounds
        },
        BoundingBox {
            x_min: 2.,
            x_max: 1.,
            ..valid_bounds
        },
        BoundingBox {
            y_min: 1.,
            y_max: 1.,
            ..valid_bounds
        },
    ] {
        assert!(MaxAtlas::new(&ink(vec![valid_record], bounds)).is_err());
    }

    assert!(MaxAtlas::new(&ink(vec![valid_record; 513], valid_bounds)).is_err());
    let excessive_width = BoundingBox {
        x_min: -20_000.,
        x_max: 20_000.,
        ..valid_bounds
    };
    assert!(MaxAtlas::new(&ink(vec![valid_record], excessive_width)).is_err());
    let excessive_area = BoundingBox {
        x_min: -5_000.,
        x_max: 5_000.,
        y_min: -5_000.,
        y_max: 5_000.,
    };
    assert!(MaxAtlas::new(&ink(vec![valid_record], excessive_area)).is_err());
}
