use super::*;
use std::str::FromStr;

#[test]
fn rich_text_preserves_whitespace_and_escapes_content_and_links() {
    let mut scene = Scene::new(Svg::new());
    let label = "  A&B <text> \"quoted\"\n🙂 ";
    let target = "https://example.test/?a=1&b=\"two\"";
    scene.scope(Text::new("").preserve_space(), |scene| {
        scene.push(TSpan::new(label));
        scene.scope(Anchor::new(target), |scene| {
            scene.push(TSpan::new("linked text"));
        });
    });
    let output = scene.finish();
    let xml = roxmltree::Document::parse(&output).unwrap();
    let text = xml.descendants().find(|n| n.has_tag_name("text")).unwrap();
    let content: String = text
        .descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect();
    assert_eq!(content, format!("{label}linked text"));
    let link = xml.descendants().find(|n| n.has_tag_name("a")).unwrap();
    assert_eq!(link.attribute("href"), Some(target));
    assert_eq!(
        xml.descendants().filter(|n| n.has_tag_name("text")).count(),
        1
    );
}

#[test]
fn replay_offsets_end_at_complete_serialized_subpaths() {
    let mut path = ReplayPath::default();
    for x in [0., 0.1234, 1000.1234] {
        path.push(
            Data::new()
                .move_to((x, 2.))
                .elliptical_arc_by((3., 3., 0., true, false, 6., 0.))
                .elliptical_arc_by((3., 3., 0., true, false, -6., 0.))
                .close(),
            true,
        );
    }
    let mut scene = Scene::new(Svg::new());
    let node = path.finish();
    scene.push(node);
    let output = scene.finish();
    let xml = roxmltree::Document::parse(&output).unwrap();
    let path = xml.descendants().find(|n| n.has_tag_name("path")).unwrap();
    let data = path.attribute("d").unwrap();
    let lengths = path
        .attribute("data-replay-lengths")
        .unwrap()
        .split(',')
        .map(|s| s.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lengths.last(), Some(&data.len()));
    for (index, end) in lengths.into_iter().enumerate() {
        let commands = svgtypes::PathParser::from(&data[..end])
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(commands.len(), (index + 1) * 4);
        assert!(matches!(
            commands.last(),
            Some(svgtypes::PathSegment::ClosePath { .. })
        ));
    }
}

#[test]
fn definitions_share_one_page_counter_across_nested_scopes() {
    let mut scene = Scene::new(Svg::new());
    let gradient = scene.definition::<Gradient>();
    scene.scope(Group::new(), |scene| {
        let mask = scene.definition::<Mask>();
        assert_ne!(gradient.id(), mask.id());
    });
    let clip = scene.definition::<Clip>();
    assert_eq!(clip.id(), "sdocx-def-2");
    assert_eq!(
        Scene::new(Svg::new()).definition::<Mask>().id(),
        "sdocx-def-0"
    );
}

#[test]
fn polyline_replay_offsets_preserve_complete_coordinate_pairs() {
    let points = [
        crate::Point {
            x: -12.345,
            y: 67.89,
        },
        crate::Point { x: 1000., y: 0. },
    ];
    let mut scene = Scene::new(Svg::new());
    scene.push(polyline(&points, true));
    let output = scene.finish();
    let xml = roxmltree::Document::parse(&output).unwrap();
    let line = xml
        .descendants()
        .find(|n| n.has_tag_name("polyline"))
        .unwrap();
    let value = line.attribute("points").unwrap();
    let offsets = line
        .attribute("data-replay-lengths")
        .unwrap()
        .split(',')
        .map(|s| s.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(offsets.last(), Some(&value.len()));
    for (index, end) in offsets.into_iter().enumerate() {
        let actual = svgtypes::PointsParser::from(&value[..end]).collect::<Vec<_>>();
        assert_eq!(actual.len(), index + 1);
        for ((x, y), expected) in actual.iter().zip(&points) {
            assert_eq!(
                *x,
                decimal(expected.x, 2)
                    .unwrap()
                    .text()
                    .parse::<f64>()
                    .unwrap()
            );
            assert_eq!(
                *y,
                decimal(expected.y, 2)
                    .unwrap()
                    .text()
                    .parse::<f64>()
                    .unwrap()
            );
        }
    }
}

#[test]
fn invalid_values_omit_the_element_without_emitting_partial_geometry() {
    let mut scene = Scene::new(Svg::new());
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        scene.push(Circle::new().cx(invalid).r(1));
        scene.push(Rectangle::new().width(invalid));
        scene.push(Path::new().data(Data::new().move_to((0, 0)).line_to((invalid, 1))));
        scene.push(polyline(
            &[
                crate::Point { x: 0., y: 0. },
                crate::Point { x: invalid, y: 1. },
            ],
            true,
        ));
        scene.scope(
            Group::new().transformed(Transform::rotate_origin(invalid)),
            |_| panic!("invalid groups must skip their children"),
        );
    }
    for opacity in [-0.01, 1.01, f64::NAN] {
        scene.push(Circle::new().r(1).fill_opacity(opacity));
    }
    scene.push(Circle::new().r(-1));
    scene.push(Line::new().stroke_width(-1));
    scene.push(Rectangle::new().height(-1));
    scene.push(Path::new().data(Data::new().move_to((0, 0)).line_to((f64::MAX, 1))));
    scene.push(
        Path::new().data(
            Data::new()
                .move_to((0, 0))
                .elliptical_arc_by((-1., 1., 0., false, true, 1., 1.)),
        ),
    );
    scene.push(Circle::new().fill(Paint::from_hex("url(#untyped)")));
    let mut replay = ReplayPath::default();
    replay.push(Data::new().move_to((0, 0)).line_to((1, 1)), true);
    replay.push(Data::new().move_to((f64::INFINITY, 0)), true);
    scene.push(replay.finish());
    scene.push(Circle::new().r(2).fill_opacity(0.5));
    let output = scene.finish();
    let xml = roxmltree::Document::parse(&output).unwrap();
    let elements: Vec<_> = xml.descendants().filter(|node| node.is_element()).collect();
    assert_eq!(elements.len(), 2);
    assert!(elements[1].has_tag_name("circle"));
    assert_eq!(elements[1].attribute("r"), Some("2"));
}

#[test]
fn typed_transforms_retain_precision_and_compose_with_group_styles() {
    let matrix = [
        1.123456789,
        -2.123456789,
        2.123456789,
        1.123456789,
        1000.123456,
        2000.123456,
    ];
    let mut scene = Scene::new(Svg::new().view_box(ViewBox::new(0., 0., 100., 100., 1)));
    scene.push(
        Group::new()
            .isolated()
            .blend(Blend::Lighten)
            .blend(Blend::Darken)
            .add(
                Circle::new()
                    .r(1)
                    .transformed(Transform::matrix(matrix, 7, 4)),
            ),
    );
    let output = scene.finish();
    let xml = roxmltree::Document::parse(&output).unwrap();
    let group = xml
        .descendants()
        .find(|node| node.has_tag_name("g"))
        .unwrap();
    assert_eq!(
        group.attribute("style"),
        Some("isolation:isolate;mix-blend-mode:darken")
    );
    let circle = xml
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .unwrap();
    let transform = circle.attribute("transform").unwrap();
    assert_eq!(
        transform,
        "matrix(1.1234568,-2.1234568,2.1234568,1.1234568,1000.1235,2000.1235)"
    );
    assert!(svgtypes::Transform::from_str(transform).is_ok());
    for transform in [
        Transform::rotate(30., 10., 20., 2),
        Transform::rotate_origin(30.),
        Transform::scale(1., 0.987654321, 9),
        Transform::translate(0., 123.45678, 4),
    ] {
        assert!(svgtypes::Transform::from_str(&transform.text().unwrap()).is_ok());
    }
}

#[test]
fn colors_and_units_reject_invalid_inputs_and_preserve_fractional_gray() {
    for invalid in [
        "#12345",
        "#1234567",
        "#+12345",
        "#12gg00",
        "red",
        "url(#paint)",
    ] {
        assert!(ColorValue::from_hex(invalid).is_none());
    }
    for invalid in [f64::NAN, -0.1, 1.1] {
        assert!(ColorValue::gray(invalid).is_none());
    }
    assert!(decimal(1., 17).is_none());
    assert!(decimal(f64::INFINITY, 2).is_none());
    let mut scene = Scene::new(Svg::new());
    let gradient = scene.definition::<Gradient>();
    scene.push(
        Definitions::new().add(
            LinearGradient::new(&gradient)
                .add(Stop::new(0., ColorValue::gray(0.07)))
                .add(Stop::new(1., ColorValue::WHITE))
                .add(Stop::new(1.1, ColorValue::BLACK)),
        ),
    );
    scene.push(Circle::new().r(1).fill(Paint::Gradient(gradient)));
    let output = scene.finish();
    let xml = roxmltree::Document::parse(&output).unwrap();
    let stops: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("stop"))
        .collect();
    assert_eq!(stops.len(), 2);
    assert_eq!(
        stops[0].attribute("stop-color"),
        Some("rgb(7.000000000%,7.000000000%,7.000000000%)")
    );
    assert_eq!(stops[1].attribute("stop-color"), Some("#ffffff"));
}
