#![cfg(feature = "render")]

#[allow(dead_code)]
mod support;

use sdocx::{Document, RenderOptions};
use support::{archive, object, page};

fn frame(kind: i16, properties: u16, fields: u32, fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 18 + fixed.len();
    [
        ((offset + flexible.len()) as u32).to_le_bytes().to_vec(),
        kind.to_le_bytes().to_vec(),
        (offset as u32).to_le_bytes().to_vec(),
        vec![2],
        properties.to_le_bytes().to_vec(),
        vec![4],
        fields.to_le_bytes().to_vec(),
        fixed.to_vec(),
        flexible.to_vec(),
    ]
    .concat()
}

fn base(visible: bool, replay_timestamp: i32) -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend(2_u16.to_le_bytes());
    fixed.extend(b"co");
    fixed.extend(1234_i64.to_le_bytes());
    fixed.extend(
        [10.0_f64, 20.0, 210.0, 100.0]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    fixed.extend(replay_timestamp.to_le_bytes());
    fixed.push(0);
    frame(0, u16::from(visible) << 3, 0, &fixed, &[])
}

fn stroke(color: u32, top_layer: bool, visible: bool, timestamp: i32) -> Vec<u8> {
    let mut channels = 2_u16.to_le_bytes().to_vec();
    channels.extend(
        [10.0_f64, 20.0, 210.0, 100.0]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    channels.extend([0.5_f32; 2].into_iter().flat_map(f32::to_le_bytes));
    channels.extend(
        [timestamp, timestamp + 10]
            .into_iter()
            .flat_map(i32::to_le_bytes),
    );
    channels.extend(1_u16.to_le_bytes());
    let mut payload = base(visible, timestamp);
    payload.extend(frame(
        1,
        (1 << 4) | (u16::from(top_layer) << 6),
        (1 << 2) | (1 << 3),
        &channels,
        &[color.to_le_bytes().to_vec(), 4.0_f32.to_le_bytes().to_vec()].concat(),
    ));
    object(1, &payload, &[])
}

fn text(label: &str, visible: bool, timestamp: i32) -> Vec<u8> {
    let mut content = (label.encode_utf16().count() as u32).to_le_bytes().to_vec();
    content.extend(label.encode_utf16().flat_map(u16::to_le_bytes));
    content.extend([0_u32; 2].into_iter().flat_map(u32::to_le_bytes));
    content.extend(
        [2.0_f32, 3.0, 4.0, 5.0]
            .into_iter()
            .flat_map(f32::to_le_bytes),
    );
    content.push(0);
    content.extend(0_u16.to_le_bytes());
    content.extend([0; 8]);
    let fields = [(content.len() as u32).to_le_bytes().to_vec(), content].concat();
    let payload = [
        base(visible, timestamp),
        frame(6, 0, 0, &[], &[]),
        frame(7, 0, 1, &[], &fields),
        frame(2, 0, 0, &[], &[]),
    ]
    .concat();
    object(2, &payload, &[])
}

fn render_layer(mut record: Vec<u8>, layer: i32) -> Vec<u8> {
    let base_offset = 7;
    let base_size = u32::from_le_bytes(record[base_offset..base_offset + 4].try_into().unwrap());
    let payload_size = u32::from_le_bytes(record[3..7].try_into().unwrap());
    record[base_offset..base_offset + 4].copy_from_slice(&(base_size + 4).to_le_bytes());
    record[3..7].copy_from_slice(&(payload_size + 4).to_le_bytes());
    record[base_offset + 14..base_offset + 18].copy_from_slice(&(1_u32 << 21).to_le_bytes());
    record.splice(
        base_offset + base_size as usize..base_offset + base_size as usize,
        layer.to_le_bytes(),
    );
    record
}

fn document(objects: Vec<Vec<u8>>) -> Document {
    sdocx::parse_bytes(&archive(&page(&[objects], 0, &[]))).unwrap()
}

fn rendered_modes(document: &Document) -> [String; 2] {
    let layout = sdocx::layout_document(document);
    let options = RenderOptions::default();
    [
        sdocx::render_layout_page_svg(document, &layout, 0, &options)
            .unwrap()
            .svg,
        sdocx::render_layout_page_replay_svg(document, &layout, 0, &options)
            .unwrap()
            .svg,
    ]
}

fn draw_order(svg: &str) -> Vec<String> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter_map(|node| {
            if ["path", "polyline", "line", "circle"]
                .into_iter()
                .any(|tag| node.has_tag_name(tag))
            {
                node.attribute("stroke")
                    .filter(|color| *color != "none")
                    .or_else(|| node.attribute("fill").filter(|color| *color != "none"))
                    .map(str::to_owned)
            } else if node.is_text()
                && node
                    .parent()
                    .is_some_and(|parent| parent.has_tag_name("tspan"))
            {
                node.text()
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_owned)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn mixed_strokes_and_text_keep_stored_order_in_export_and_replay() {
    let document = document(vec![
        stroke(0xffff0000, false, true, 3000),
        text("between strokes", true, 2000),
        stroke(0xff0000ff, false, true, 1000),
    ]);
    for svg in rendered_modes(&document) {
        assert_eq!(draw_order(&svg), ["#ff0000", "between strokes", "#0000ff"]);
    }
}

#[test]
fn mixed_container_children_keep_their_place_between_root_objects() {
    let nested = object(4, &base(true, 3000), &[text("nested child", true, 5000)]);
    let container = object(
        4,
        &base(true, 1000),
        &[
            text("first child", true, 6000),
            stroke(0xff0000ff, false, true, 4000),
            nested,
        ],
    );
    let document = document(vec![
        stroke(0xffff0000, false, true, 7000),
        container,
        stroke(0xff00ff00, false, true, 2000),
    ]);
    for svg in rendered_modes(&document) {
        assert_eq!(
            draw_order(&svg),
            [
                "#ff0000",
                "first child",
                "#0000ff",
                "nested child",
                "#00ff00"
            ]
        );
    }
}

#[test]
fn hidden_objects_and_containers_do_not_disrupt_visible_order() {
    let hidden_container = object(
        4,
        &base(false, 1000),
        &[
            stroke(0xffff00ff, false, true, 1000),
            text("hidden container child", true, 1000),
        ],
    );
    let document = document(vec![
        stroke(0xffff0000, false, true, 3000),
        text("hidden text", false, 2000),
        hidden_container,
        text("visible text", true, 2000),
        stroke(0xff00ffff, false, false, 1000),
        stroke(0xff0000ff, false, true, 1000),
    ]);
    for svg in rendered_modes(&document) {
        assert_eq!(draw_order(&svg), ["#ff0000", "visible text", "#0000ff"]);
    }
}

#[test]
fn root_top_layer_strokes_keep_relative_order_in_the_final_pass() {
    let document = document(vec![
        stroke(0xffffff00, true, true, 4000),
        stroke(0xffff0000, false, true, 3000),
        text("under top layer", true, 2000),
        stroke(0xff00ffff, true, true, 1000),
        stroke(0xff0000ff, false, true, 500),
    ]);
    for (mode, svg) in rendered_modes(&document).into_iter().enumerate() {
        assert_eq!(
            draw_order(&svg),
            [
                "#ff0000",
                "under top layer",
                "#0000ff",
                "#ffff00",
                "#00ffff"
            ]
        );
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let replay_indices = xml
            .descendants()
            .filter_map(|node| node.attribute("data-replay-stroke"))
            .collect::<Vec<_>>();
        if mode == 1 {
            assert_eq!(replay_indices, ["1", "3", "0", "2"]);
        }
        let top_pass = xml
            .descendants()
            .find(|node| {
                node.attribute("style")
                    .is_some_and(|style| style.contains("mix-blend-mode:darken"))
            })
            .unwrap();
        let colors = top_pass
            .descendants()
            .filter_map(|node| node.attribute("stroke"))
            .collect::<Vec<_>>();
        assert_eq!(colors, ["#ffff00", "#00ffff"]);
    }
}

#[test]
fn render_passes_preserve_root_order_and_top_pen_overrides_common_layer() {
    let document = document(vec![
        render_layer(stroke(0xffff00ff, false, true, 6000), 2),
        render_layer(stroke(0xff0000ff, false, true, 5000), 1),
        render_layer(stroke(0xffff0000, false, true, 4000), 0),
        render_layer(text("masking text", true, 3000), 2),
        render_layer(stroke(0xff00ffff, true, true, 2000), 2),
        text("base text", true, 1000),
    ]);
    for svg in rendered_modes(&document) {
        assert_eq!(
            draw_order(&svg),
            [
                "#ff0000",
                "base text",
                "#0000ff",
                "#00ffff",
                "#ff00ff",
                "masking text"
            ]
        );
    }
}

#[test]
fn top_pass_excludes_nonstrokes_and_unknown_render_layers_are_not_aliases() {
    let top_container = render_layer(
        object(
            4,
            &base(true, 1000),
            &[text("top container child", true, 1000)],
        ),
        1,
    );
    let document = document(vec![
        render_layer(stroke(0xff999999, false, true, 1000), -1),
        render_layer(stroke(0xff777777, false, true, 1000), 3),
        render_layer(stroke(0xff555555, false, true, 1000), 32),
        render_layer(text("top text", true, 1000), 1),
        render_layer(text("unknown text", true, 1000), 32),
        top_container,
        stroke(0xffff0000, false, true, 1000),
    ]);
    for svg in rendered_modes(&document) {
        assert_eq!(draw_order(&svg), ["#ff0000"]);
    }
}

#[test]
fn container_children_follow_the_selected_root_pass_without_refiltering() {
    let container = object(
        4,
        &base(true, 3000),
        &[
            render_layer(stroke(0xffffff00, true, true, 6000), 2),
            render_layer(text("container top text", true, 5000), 1),
            render_layer(stroke(0xff0000ff, false, true, 4000), 1),
        ],
    );
    let document = document(vec![
        stroke(0xffff0000, false, true, 7000),
        container,
        stroke(0xff00ff00, false, true, 2000),
        stroke(0xff00ffff, true, true, 1000),
    ]);
    for svg in rendered_modes(&document) {
        assert_eq!(
            draw_order(&svg),
            [
                "#ff0000",
                "#ffff00",
                "container top text",
                "#0000ff",
                "#00ff00",
                "#00ffff"
            ]
        );
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let top_pass = xml
            .descendants()
            .find(|node| {
                node.attribute("style")
                    .is_some_and(|style| style.contains("mix-blend-mode:darken"))
            })
            .unwrap();
        let colors = top_pass
            .descendants()
            .filter_map(|node| node.attribute("stroke"))
            .collect::<Vec<_>>();
        assert_eq!(colors, ["#00ffff"]);
    }
}

#[test]
fn rejected_roots_and_containers_leave_dense_replay_indices_with_source_mapping() {
    let rejected_container = render_layer(
        object(
            4,
            &base(true, 1000),
            &[stroke(0xff777777, false, true, 1000)],
        ),
        1,
    );
    let accepted_container = object(
        4,
        &base(true, 2000),
        &[
            stroke(0xffff0000, false, true, 2000),
            text("between accepted children", true, 2000),
            stroke(0xff0000ff, false, true, 2000),
        ],
    );
    let raw = page(
        &[vec![
            render_layer(stroke(0xff999999, false, true, 1000), -1),
            rejected_container,
            stroke(0xffffff00, true, true, 2000),
            accepted_container,
            stroke(0xff00ff00, false, true, 3000),
        ]],
        0,
        &[],
    );
    let parsed = sdocx::parse_bytes_detailed(&archive(&raw)).unwrap();
    let roots = &parsed.stored_pages[0].page.layers.layers[0].objects;
    let expected_offsets = [
        roots[2].payload_offset,
        roots[3].children[0].payload_offset,
        roots[3].children[2].payload_offset,
        roots[4].payload_offset,
    ];
    let accepted = parsed.document.pages[0]
        .composed_strokes()
        .map(|(object, stroke)| {
            let color = stroke.color.unwrap();
            (object.source_offset.unwrap(), (color.r, color.g, color.b))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        accepted,
        expected_offsets
            .into_iter()
            .zip([(255, 255, 0), (255, 0, 0), (0, 0, 255), (0, 255, 0)])
            .collect::<Vec<_>>()
    );
    let [_, replay] = rendered_modes(&parsed.document);
    let xml = roxmltree::Document::parse(&replay).unwrap();
    let rendered_strokes = xml
        .descendants()
        .filter_map(|node| {
            let index = node
                .attribute("data-replay-stroke")?
                .parse::<usize>()
                .unwrap();
            let color = node
                .descendants()
                .find_map(|child| child.attribute("stroke"))
                .unwrap();
            Some((index, color))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rendered_strokes,
        [
            (1, "#ff0000"),
            (2, "#0000ff"),
            (3, "#00ff00"),
            (0, "#ffff00")
        ]
    );
    let mut indices = rendered_strokes
        .into_iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    assert_eq!(indices, (0..accepted.len()).collect::<Vec<_>>());
}

#[test]
fn nested_masking_containers_keep_top_and_unknown_children_inline() {
    let nested = render_layer(
        object(
            4,
            &base(true, 3000),
            &[
                render_layer(stroke(0xffffff00, true, true, 3000), 1),
                render_layer(stroke(0xff0000ff, false, true, 3000), -1),
                render_layer(text("unknown child in masking", true, 3000), 32),
            ],
        ),
        -1,
    );
    let masking = render_layer(
        object(
            4,
            &base(true, 3000),
            &[text("first masking child", true, 3000), nested],
        ),
        2,
    );
    let document = document(vec![
        masking,
        stroke(0xffff0000, false, true, 2000),
        stroke(0xff00ffff, true, true, 1000),
    ]);
    for svg in rendered_modes(&document) {
        assert_eq!(
            draw_order(&svg),
            [
                "#ff0000",
                "#00ffff",
                "first masking child",
                "#ffff00",
                "#0000ff",
                "unknown child in masking"
            ]
        );
    }
}
