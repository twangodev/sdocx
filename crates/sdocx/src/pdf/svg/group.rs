use std::{collections::HashMap, collections::hash_map::Entry, sync::Arc};

use krilla::num::NormalizedF32;
use krilla::surface::Surface;
use usvg::{BlendMode, Node, Opacity};

use super::util::{UsvgTransformExt, convert_blend_mode};
use super::{ProcessContext, SvgError, clip_path, filter, image, mask, path};

// Only producer-marked, effect-free solid path blocks are eligible. The key is
// made from the resolved path data and fill, never from the SVG identifier.
fn reuse_key(group: &usvg::Group, budget: usize) -> Option<Vec<u32>> {
    fn paths(
        group: &usvg::Group,
        key: &mut Vec<u32>,
        count: &mut usize,
        depth: usize,
        budget: usize,
        visited: &mut usize,
    ) -> Option<()> {
        *visited += 1;
        if depth > 8 || key.len() > budget || *count > 32768 || *visited > 32768 {
            return None;
        }
        if !group.transform().is_identity()
            || group.opacity() != Opacity::ONE
            || group.blend_mode() != BlendMode::Normal
            || group.isolate()
            || group.clip_path().is_some()
            || group.mask().is_some()
            || !group.filters().is_empty()
        {
            return None;
        }
        for child in group.children() {
            *visited += 1;
            if *visited > 32768 {
                return None;
            }
            match child {
                Node::Group(g) => paths(g, key, count, depth + 1, budget, visited)?,
                Node::Path(p) if p.is_visible() && p.stroke().is_none() => {
                    let fill = p.fill()?;
                    let usvg::Paint::Color(color) = fill.paint() else {
                        return None;
                    };
                    key.extend([
                        0,
                        u32::from(color.red),
                        u32::from(color.green),
                        u32::from(color.blue),
                        fill.opacity().get().to_bits(),
                        match fill.rule() {
                            usvg::FillRule::NonZero => 0,
                            usvg::FillRule::EvenOdd => 1,
                        },
                    ]);
                    for segment in p.data().segments() {
                        if key.len() + 8 > budget {
                            return None;
                        }
                        use tiny_skia::PathSegment::*;
                        let (op, points): (u32, &[tiny_skia::Point]) = match &segment {
                            MoveTo(p) => (1, std::slice::from_ref(p)),
                            LineTo(p) => (2, std::slice::from_ref(p)),
                            QuadTo(..) | CubicTo(..) | Close => (0, &[]),
                        };
                        if op != 0 {
                            key.push(op);
                            for p in points {
                                key.extend([p.x.to_bits(), p.y.to_bits()]);
                            }
                        } else {
                            match segment {
                                QuadTo(a, b) => key.extend([
                                    3,
                                    a.x.to_bits(),
                                    a.y.to_bits(),
                                    b.x.to_bits(),
                                    b.y.to_bits(),
                                ]),
                                CubicTo(a, b, c) => key.extend([
                                    4,
                                    a.x.to_bits(),
                                    a.y.to_bits(),
                                    b.x.to_bits(),
                                    b.y.to_bits(),
                                    c.x.to_bits(),
                                    c.y.to_bits(),
                                ]),
                                Close => key.push(5),
                                _ => unreachable!(),
                            }
                        }
                    }
                    key.push(6);
                    *count += 1;
                }
                _ => return None,
            }
        }
        Some(())
    }
    if !group.id().starts_with("sdocx-montblanc-reuse-") {
        return None;
    }
    let mut key = Vec::new();
    let mut count = 0;
    paths(group, &mut key, &mut count, 0, budget, &mut 0)?;
    (count >= 32 && key.len() <= budget).then_some(key)
}

/// Candidate fingerprints and emitted graphics belong to one SVG traversal.
/// Equal fingerprints share their backing storage across all node occurrences.
#[derive(Default)]
pub(super) struct ReuseCache {
    pub(super) nodes: HashMap<usize, Arc<[u32]>>,
    pub(super) graphics: HashMap<Arc<[u32]>, krilla::graphic::Graphic>,
}

pub(super) fn collect_reuse_candidates(root: &usvg::Group) -> ReuseCache {
    let mut counts = HashMap::<Arc<[u32]>, usize>::new();
    let mut nodes = HashMap::new();
    // Markers may come from arbitrary SVG input. Limit all fingerprint work
    // and storage, and fall back to normal rendering when a limit is reached.
    let mut work = vec![root];
    let mut remaining = 2_000_000usize;
    let mut visited = 0usize;
    let mut markers = 0usize;
    while let Some(group) = work.pop() {
        visited += 1;
        if visited > 100_000 {
            break;
        }
        if group.id().starts_with("sdocx-montblanc-reuse-") {
            markers += 1;
            if markers > 128 || remaining == 0 {
                break;
            }
            if let Some(key) = reuse_key(group, remaining) {
                remaining -= key.len();
                let key = match counts.entry(Arc::from(key)) {
                    Entry::Occupied(mut entry) => {
                        *entry.get_mut() += 1;
                        entry.key().clone()
                    }
                    Entry::Vacant(entry) => {
                        let key = entry.key().clone();
                        entry.insert(1);
                        key
                    }
                };
                nodes.insert(group as *const _ as usize, key);
            } else {
                // Failed fingerprints can have consumed the whole allowance.
                // Stop scanning rather than repeatedly processing hostile input.
                break;
            }
        }
        let slots = 100_000usize.saturating_sub(visited + work.len());
        work.extend(
            group
                .children()
                .iter()
                .filter_map(|child| match child {
                    Node::Group(g) => Some(g.as_ref()),
                    _ => None,
                })
                .take(slots),
        );
    }
    nodes.retain(|_, key| counts.get(key).copied().unwrap_or(0) >= 2);
    ReuseCache {
        nodes,
        graphics: HashMap::new(),
    }
}

pub(super) fn reuse_transform_is_safe(ctm: krilla::geom::Transform) -> bool {
    ctm.kx() == 0.0
        && ctm.ky() == 0.0
        && ctm.sx().is_finite()
        && ctm.sy().is_finite()
        && ctm.tx().is_finite()
        && ctm.ty().is_finite()
        && ctm.sx() != 0.0
        && ctm.sy() != 0.0
}

pub(super) fn render(
    group: &usvg::Group,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) {
    if process_context.error.is_some() {
        return;
    }
    fn contains_native_ink(group: &usvg::Group, context: &ProcessContext) -> bool {
        context.has_native_ink(group)
            || group.children().iter().any(|child| match child {
                Node::Group(child) => contains_native_ink(child, context),
                _ => false,
            })
    }
    // Include caller transforms as well as SVG ancestors. Some PDF readers
    // round rotated or sheared paths differently across a form boundary.
    let axis_aligned = process_context.reuse_base_transform_safe
        && reuse_transform_is_safe(group.abs_transform().to_krilla())
        && reuse_transform_is_safe(surface.ctm());
    if let Some(key) = process_context
        .reuse
        .nodes
        .get(&(group as *const _ as usize))
        .cloned()
        && axis_aligned
        && !contains_native_ink(group, process_context)
    {
        if let Some(graphic) = process_context.reuse.graphics.get(&key) {
            surface.push_blend_mode(convert_blend_mode(&BlendMode::Normal));
            surface.draw_graphic(graphic.clone());
            surface.pop();
            return;
        }
        let viewport = process_context.cull_viewport.take();
        let mut builder = surface.stream_builder();
        {
            let mut sub = builder.surface();
            for child in group.children() {
                render_node(child, &mut sub, process_context);
            }
            sub.finish();
        }
        process_context.cull_viewport = viewport;
        let graphic = krilla::graphic::Graphic::new(builder.finish(), false);
        surface.push_blend_mode(convert_blend_mode(&BlendMode::Normal));
        surface.draw_graphic(graphic.clone());
        surface.pop();
        process_context.reuse.graphics.insert(key, graphic);
        return;
    }
    let native_ink = process_context.has_native_ink(group);
    if !group.filters().is_empty() && !native_ink {
        filter::render(group, surface, process_context);
        return;
    }

    let mut pop_count = 0;

    surface.push_transform(&group.transform().to_krilla());
    pop_count += 1;

    if let Some(clip_path) = group.clip_path() {
        pop_count += clip_path::render(group, clip_path, surface, process_context);
    }

    if let Some(mask) = group.mask() {
        pop_count += mask::render(mask, surface, process_context);
    }

    surface.push_blend_mode(convert_blend_mode(&group.blend_mode()));
    surface.push_opacity(NormalizedF32::new(group.opacity().get()).unwrap());
    pop_count += 2;

    // There is the `group.should_isolate` method which also returns true if
    // there is a clip path, since strictly speaking, just a clip path
    // also requires layer isolation. However, clip paths are very common
    // and it would be wasteful to generate new isolated XObjects for that
    // (especially since it shouldn't make any visual difference), so we
    // don't include this here.
    let should_isolate = group.isolate()
        || group.opacity() != Opacity::ONE
        || group.mask().is_some()
        || group.blend_mode() != BlendMode::Normal;

    if should_isolate {
        surface.push_isolated();
        pop_count += 1
    }

    if native_ink {
        process_context.render_native_ink(group, surface);
    } else {
        for child in group.children() {
            if process_context.error.is_some() {
                break;
            }
            render_node(child, surface, process_context);
        }
    }

    for _ in 0..pop_count {
        surface.pop();
    }
}

pub(super) fn render_node(
    node: &Node,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) {
    if process_context.error.is_some() {
        return;
    }
    if process_context.should_cull_node(node) {
        return;
    }
    match node {
        Node::Group(g) => render(g, surface, process_context),
        Node::Path(p) => path::render(p, surface, process_context),
        Node::Image(i) => {
            if let Err(message) = image::render(i, surface, process_context)
                && process_context.svg_settings.reject_images
            {
                process_context.error = Some(SvgError::Image(message));
            }
        }
        Node::Text(t) => process_context.render_text(t, surface),
    }
}

#[cfg(test)]
mod reuse_tests {
    use crate::pdf::svg::{SurfaceExt, SvgSettings};
    use krilla::{Document, geom::Size, page::PageSettings};

    fn svg(marked: bool, distinct: bool, one: bool, internal: &str) -> String {
        let paths: String = (0..40)
            .map(|i| format!("<path d='M {i} 1 l 3 0 l 0 3 z' fill-opacity='.3'/>"))
            .collect();
        let marker = |i| {
            if marked {
                format!("sdocx-montblanc-reuse-{i}")
            } else {
                format!("ordinary-{i}")
            }
        };
        let fill = if distinct { "blue" } else { "red" };
        let second = if one {
            String::new()
        } else {
            format!(
                "<g transform='translate(2 8)' opacity='.7' clip-path='url(#c)' fill='{fill}'><g id='{}' {internal}>{paths}</g></g>",
                marker(2)
            )
        };
        format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><defs><clipPath id='c'><rect x='0' y='0' width='35' height='40'/></clipPath></defs><rect width='100' height='50' fill='white'/><g fill='red' opacity='.7' clip-path='url(#c)' transform='translate(2 8)'><g id='{}'>{paths}</g></g><path d='M0 30h80v2H0z' fill='green'/>{second}<path d='M0 33h80v2H0z' fill='black'/></svg>",
            marker(1)
        )
    }
    fn pdf(svg: &str) -> Vec<u8> {
        pdf_transformed(svg, krilla::geom::Transform::identity())
    }
    fn pdf_transformed(svg: &str, transform: krilla::geom::Transform) -> Vec<u8> {
        let tree = usvg::Tree::from_str(svg, &Default::default()).unwrap();
        let mut doc = Document::new();
        let size = Size::from_wh(100., 50.).unwrap();
        let mut page = doc.start_page_with(PageSettings::new(size));
        let mut surface = page.surface();
        surface.push_transform(&transform);
        surface
            .draw_svg(&tree, size, SvgSettings::default())
            .unwrap();
        surface.pop();
        surface.finish();
        page.finish();
        doc.finish().unwrap()
    }
    fn form_count(bytes: &[u8]) -> usize {
        let pdf = lopdf::Document::load_mem(bytes).unwrap();
        pdf.objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter(|s| {
                s.dict
                    .get(b"Subtype")
                    .and_then(|o| o.as_name())
                    .is_ok_and(|n| n == b"Form")
            })
            .count()
    }
    #[test]
    fn matching_candidates_share_fingerprint_storage_after_counting() {
        let tree = usvg::Tree::from_str(&svg(true, false, false, ""), &Default::default()).unwrap();
        let cache = super::collect_reuse_candidates(tree.root());
        assert_eq!(cache.nodes.len(), 2);
        let keys: Vec<_> = cache.nodes.values().collect();
        assert!(std::sync::Arc::ptr_eq(keys[0], keys[1]));
        // Only the two node occurrences retain the fingerprint; counting is
        // finished and no rendered graphic has been cached yet.
        assert_eq!(std::sync::Arc::strong_count(keys[0]), 2);
        assert!(cache.graphics.is_empty());
    }

    #[test]
    fn marked_blocks_require_matching_resolved_content_and_preserve_mixed_order() {
        let shared = pdf(&svg(true, false, false, ""));
        let flat = pdf(&svg(false, false, false, ""));
        assert!(form_count(&shared) > form_count(&flat));
        let duplicate_ids = svg(true, false, false, "")
            .replace("sdocx-montblanc-reuse-2", "sdocx-montblanc-reuse-1");
        assert_eq!(form_count(&pdf(&duplicate_ids)), form_count(&shared));
        assert_eq!(
            form_count(&pdf(&svg(true, true, false, ""))),
            form_count(&pdf(&svg(false, true, false, "")))
        );
        assert_eq!(
            form_count(&pdf(&svg(true, false, true, ""))),
            form_count(&pdf(&svg(false, false, true, "")))
        );
        assert_eq!(
            form_count(&pdf(&svg(true, false, false, "opacity='.8'"))),
            form_count(&pdf(&svg(false, false, false, "opacity='.8'")))
        );
        let make = |marked| {
            svg(marked, false, false, "")
                .replacen("translate(2 8)", "translate(80 8)", 1)
                .replace(
                    "<rect width='100'",
                    "<g style='mix-blend-mode:multiply'><rect width='100'",
                )
                .replace("</svg>", "</g></svg>")
        };
        let _ = pdf(&make(true));
        let _ = pdf(&make(false));
        let mask = |marked| {
            svg(marked, false, false, "").replace("</defs>", "<mask id='m'><rect width='100' height='50' fill='white' opacity='.6'/></mask></defs>").replace("<rect width='100' height='50' fill='white'/>", "<g mask='url(#m)' style='mix-blend-mode:multiply'><rect width='100' height='50' fill='white'/>").replace("</svg>", "</g></svg>")
        };
        let _ = pdf(&mask(true));
        let _ = pdf(&mask(false));
    }
    #[test]
    fn rotated_or_sheared_ancestors_disable_reuse() {
        let original = svg(true, false, false, "");
        let unmarked = original.replace("sdocx-montblanc-reuse-", "ordinary-");
        for transform in ["rotate(7 50 25)", "skewX(12)", "translate(2 3) rotate(7)"] {
            let wrap = |input: &str| {
                input
                    .replace(
                        "<rect width='100'",
                        &format!("<g transform='{transform}'><rect width='100'"),
                    )
                    .replace("</svg>", "</g></svg>")
            };
            assert_eq!(
                form_count(&pdf(&wrap(&original))),
                form_count(&pdf(&wrap(&unmarked)))
            );
        }
        assert!(!super::reuse_transform_is_safe(
            krilla::geom::Transform::from_row(f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0)
        ));
        assert!(!super::reuse_transform_is_safe(
            krilla::geom::Transform::from_scale(0.0, 1.0)
        ));
        let rotation = krilla::geom::Transform::from_rotate(7.0);
        assert_eq!(
            form_count(&pdf_transformed(&original, rotation)),
            form_count(&pdf_transformed(&unmarked, rotation))
        );
        // Net identity across nested SVG transforms remains eligible.
        let cancel = |input: &str| {
            input
                .replace(
                    "<rect width='100'",
                    "<g transform='rotate(90)'><g transform='rotate(-90)'><rect width='100'",
                )
                .replace("</svg>", "</g></g></svg>")
        };
        assert!(form_count(&pdf(&cancel(&original))) > form_count(&pdf(&cancel(&unmarked))));
        // Reflection and axis-aligned scaling retain reuse.
        let wrap = |input: &str| {
            input
                .replace(
                    "<rect width='100'",
                    "<g transform='translate(100 0) scale(-1 2)'><rect width='100'",
                )
                .replace("</svg>", "</g></svg>")
        };
        assert!(form_count(&pdf(&wrap(&original))) > form_count(&pdf(&wrap(&unmarked))));
    }
    #[test]
    fn registered_native_ink_descendant_is_never_reused_or_skipped() {
        let paths: String = (0..40)
            .map(|i| format!("<path d='M {i} 1 l 3 0 l 0 3 z'/>"))
            .collect();
        let xml = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><g id='sdocx-montblanc-reuse-a'><g id='native'>{paths}</g></g><g id='sdocx-montblanc-reuse-b'>{paths}</g></svg>"
        );
        let tree = usvg::Tree::from_str(&xml, &Default::default()).unwrap();
        let mut doc = Document::new();
        let size = Size::from_wh(100., 50.).unwrap();
        let mut page = doc.start_page_with(PageSettings::new(size));
        let mut surface = page.surface();
        let ids = std::collections::HashSet::from(["native".to_string()]);
        let mut text_hook = |_: &usvg::Text, _: &mut krilla::surface::Surface<'_>| Ok(false);
        let mut ink_hook = |_: &usvg::Group, _: &mut krilla::surface::Surface<'_>| {
            Err("native hook invoked".to_string())
        };
        let result = surface.draw_svg_with_text_and_ink(
            &tree,
            size,
            SvgSettings::default(),
            &mut text_hook,
            &ids,
            &mut ink_hook,
        );
        assert!(
            matches!(result, Err(crate::pdf::svg::SvgError::Ink(message)) if message == "native hook invoked")
        );
        surface.finish();
        page.finish();
    }
    #[test]
    fn hostile_reuse_markers_are_bounded() {
        let paths: String = (0..40)
            .map(|i| format!("<path d='M {i} 1 l 3 0 l 0 3 z'/>"))
            .collect();
        let nested = format!("{}{}{}", "<g>".repeat(12), paths, "</g>".repeat(12));
        let xml = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50'><g id='sdocx-montblanc-reuse-x'>{nested}</g></svg>"
        );
        let tree = usvg::Tree::from_str(&xml, &Default::default()).unwrap();
        let usvg::Node::Group(group) = tree.node_by_id("sdocx-montblanc-reuse-x").unwrap() else {
            panic!("expected group")
        };
        let mut retained_depth = 0;
        let mut current = group.as_ref();
        while let [usvg::Node::Group(child)] = current.children() {
            retained_depth += 1;
            current = child;
        }
        assert!(retained_depth > 8, "fixture must retain nested groups");
        assert!(super::reuse_key(group, 2_000_000).is_none());
        let simple =
            usvg::Tree::from_str(&svg(true, false, false, ""), &Default::default()).unwrap();
        let usvg::Node::Group(group) = simple.node_by_id("sdocx-montblanc-reuse-1").unwrap() else {
            panic!("expected group")
        };
        assert!(super::reuse_key(group, 16).is_none());
    }
}
