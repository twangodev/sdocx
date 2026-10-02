use super::*;

#[test]
fn original_page_objects_withdraw_and_restore_native_clip_provenance() {
    let mut document = crate::parse_bytes(include_bytes!(
        "../../tests/fixtures/native_body_table.sdocx"
    ))
    .unwrap();
    let fonts = fonts::FontBook::default();
    let options = RenderOptions::default();
    let mut cache = DocumentTextCache::default();
    let mut previous = None;

    for empty in [true, false, true] {
        document.pages[0].objects = if empty {
            Vec::new()
        } else {
            vec![PageObject {
                render_layer: crate::ObjectRenderLayer::Base,
                source_offset: None,
                content: PageObjectContent::Container(Vec::new()),
            }]
        };
        let layout = layout_document(&document);
        let cached = cache
            .render_layout_page_svg(&document, &layout, 0, &options, &fonts)
            .unwrap();
        let cold =
            render_layout_page_svg_with_fonts(&document, &layout, 0, &options, &fonts).unwrap();
        assert_eq!(cached, cold);
        let plan = Rc::clone(&cache.plans[0].1);
        assert_eq!(plan.layout.native_object_entry.is_some(), empty);
        if let Some(previous) = &previous {
            assert!(!Rc::ptr_eq(&plan, previous));
        }
        assert_eq!(
            cache
                .context
                .as_ref()
                .unwrap()
                .metrics
                .native_object_page_obstacles
                .is_some(),
            empty
        );
        let xml = roxmltree::Document::parse(&cached.svg).unwrap();
        let text_clips = xml
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| {
                node.ancestors()
                    .filter(|node| node.attribute("clip-path").is_some())
                    .count()
                    .saturating_sub(1)
            })
            .sum::<usize>();
        assert_eq!(text_clips, if empty { 2 } else { 6 });
        let repeated = cache
            .render_layout_page_svg(&document, &layout, 0, &options, &fonts)
            .unwrap();
        assert_eq!(cached, repeated);
        assert!(Rc::ptr_eq(&plan, &cache.plans[0].1));
        previous = Some(plan);
    }
}
