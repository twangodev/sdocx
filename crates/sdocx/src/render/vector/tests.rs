use super::*;

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
