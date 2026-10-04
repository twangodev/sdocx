use crate::{ObjectRenderLayer, PageObject, PageObjectContent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderPass {
    Base,
    Top,
    Masking,
}

impl PageObject {
    pub(crate) fn render_pass(&self) -> Option<RenderPass> {
        let stroke = match &self.content {
            PageObjectContent::Stroke(stroke) => Some(stroke),
            _ => None,
        };
        if stroke.is_some_and(|stroke| {
            stroke
                .rendering
                .as_ref()
                .is_some_and(|rendering| rendering.properties.top_layer_pen)
        }) {
            return Some(RenderPass::Top);
        }
        match self.render_layer {
            ObjectRenderLayer::Base => Some(RenderPass::Base),
            ObjectRenderLayer::Top if stroke.is_some() => Some(RenderPass::Top),
            ObjectRenderLayer::Masking => Some(RenderPass::Masking),
            ObjectRenderLayer::Top
            | ObjectRenderLayer::Other(_)
            | ObjectRenderLayer::Unresolved => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoundingBox, PageElement, Stroke, StrokeProperties, StrokeRendering, StrokeStyle};

    fn stroke(top_layer_pen: bool) -> PageObjectContent {
        PageObjectContent::Stroke(Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: None,
                advanced_settings: None,
                tool_type_raw: 2,
                properties: StrokeProperties {
                    compressed: false,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle::default(),
            }),
            bbox: BoundingBox::default(),
            points: Vec::new(),
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: 1.0,
        })
    }

    fn object(render_layer: ObjectRenderLayer, content: PageObjectContent) -> PageObject {
        PageObject {
            render_layer,
            source_offset: None,
            content,
        }
    }

    #[test]
    fn native_root_selection_excludes_non_strokes_from_the_top_pass() {
        for content in [
            stroke(false),
            PageObjectContent::Element(PageElement::Image {
                bbox: BoundingBox::default(),
                media_index: 0,
            }),
            PageObjectContent::Container(Vec::new()),
        ] {
            let expected_top =
                matches!(content, PageObjectContent::Stroke(_)).then_some(RenderPass::Top);
            for (layer, expected) in [
                (ObjectRenderLayer::Base, Some(RenderPass::Base)),
                (ObjectRenderLayer::Top, expected_top),
                (ObjectRenderLayer::Masking, Some(RenderPass::Masking)),
                (ObjectRenderLayer::Other(-1), None),
                (ObjectRenderLayer::Other(3), None),
                (ObjectRenderLayer::Other(32), None),
                (ObjectRenderLayer::Unresolved, None),
            ] {
                assert_eq!(object(layer, content.clone()).render_pass(), expected);
            }
        }
    }

    #[test]
    fn top_layer_pen_overrides_conflicting_and_unknown_render_ids() {
        for layer in [
            ObjectRenderLayer::Base,
            ObjectRenderLayer::Top,
            ObjectRenderLayer::Masking,
            ObjectRenderLayer::Other(-1),
            ObjectRenderLayer::Other(3),
            ObjectRenderLayer::Unresolved,
        ] {
            assert_eq!(
                object(layer, stroke(true)).render_pass(),
                Some(RenderPass::Top)
            );
        }
    }

    #[test]
    fn container_selection_does_not_reclassify_its_children() {
        let children = vec![object(ObjectRenderLayer::Masking, stroke(true))];
        for layer in [ObjectRenderLayer::Base, ObjectRenderLayer::Masking] {
            let container = object(layer, PageObjectContent::Container(children.clone()));
            let expected = if layer == ObjectRenderLayer::Base {
                RenderPass::Base
            } else {
                RenderPass::Masking
            };
            assert_eq!(container.render_pass(), Some(expected));
        }
    }

    #[test]
    fn stroke_without_native_rendering_uses_its_common_render_layer() {
        let PageObjectContent::Stroke(mut stroke) = stroke(false) else {
            unreachable!();
        };
        stroke.rendering = None;
        assert_eq!(
            PageObject::from(stroke.clone()).render_pass(),
            Some(RenderPass::Base)
        );
        assert_eq!(
            object(ObjectRenderLayer::Top, PageObjectContent::Stroke(stroke)).render_pass(),
            Some(RenderPass::Top)
        );
    }
}
