use crate::render::text::native_cell_clip::{NativeCellClipError, NativeCellRunClip};
use crate::render::vector::{Clip, ClipPath, Definitions, Group, Rectangle, Scene, Styled};

#[cfg(all(test, feature = "pdf"))]
mod pdf_tests;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NativeRunClip {
    rectangle: Option<ClipRectangle>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ClipRectangle {
    origin: [f64; 2],
    size: [f64; 2],
}

impl NativeRunClip {
    pub fn new(
        clip: NativeCellRunClip,
        output_translation: [f64; 2],
    ) -> Result<Self, NativeCellClipError> {
        if !output_translation.into_iter().all(f64::is_finite) {
            return Err(NativeCellClipError);
        }
        let NativeCellRunClip::Clipped { world_rect, .. } = clip else {
            return Ok(Self { rectangle: None });
        };
        let [left, top, right, bottom] = world_rect.map(f64::from);
        if !world_rect.into_iter().all(f32::is_finite) {
            return Err(NativeCellClipError);
        }
        if right <= left || bottom <= top {
            return Ok(Self { rectangle: None });
        }
        let rectangle = ClipRectangle {
            origin: [left + output_translation[0], top + output_translation[1]],
            size: [right - left, bottom - top],
        };
        if (0..2).any(|axis| {
            let far_edge = rectangle.origin[axis] + rectangle.size[axis];
            !far_edge.is_finite() || far_edge <= rectangle.origin[axis]
        }) {
            return Err(NativeCellClipError);
        }
        Ok(Self {
            rectangle: Some(rectangle),
        })
    }

    pub fn paint(self, scene: &mut Scene, draw: impl FnOnce(&mut Scene)) {
        let Some(rectangle) = self.rectangle else {
            draw(scene);
            return;
        };
        let id = scene.definition::<Clip>();
        let node = Rectangle::new()
            .x(rectangle.origin[0])
            .y(rectangle.origin[1])
            .width(rectangle.size[0])
            .height(rectangle.size[1]);
        scene.push(Definitions::new().add(ClipPath::new(&id).add(node)));
        scene.scope(Group::new().clipped(&id), draw);
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::render::vector::{Svg, Text};
    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    #[derive(Deserialize)]
    struct Capture {
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        world_clip: Option<[f32; 4]>,
        backend_clip: bool,
    }

    #[test]
    fn captured_clip_transport_preserves_text_and_restores_neighbor_scopes() {
        let bytes = include_bytes!("../../../../../../../conformance/table-text-clip-paths.json");
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "e5f6d442e39aedf995d807a276d86ab7ff877c31d044a5597b620adb4eaa6bdc"
        );
        let capture: Capture = serde_json::from_slice(bytes).unwrap();
        assert_eq!(capture.cases.len(), 132);
        for case in capture.cases {
            let mut scene = Scene::new(Svg::new());
            scene.push(Text::new("before"));
            let clip = case
                .world_clip
                .map_or(NativeCellRunClip::Unclipped, |world_rect| {
                    NativeCellRunClip::Clipped {
                        world_rect,
                        intersects_source: false,
                    }
                });
            let shift = [11.125, 23.375];
            NativeRunClip::new(clip, shift)
                .unwrap()
                .paint(&mut scene, |scene| scene.push(Text::new("source")));
            scene.push(Text::new("after"));
            let svg = scene.finish();
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let text = xml
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .collect::<Vec<_>>();
            assert_eq!(
                text.iter()
                    .map(|node| node.text().unwrap())
                    .collect::<Vec<_>>(),
                ["before", "source", "after"],
                "{}",
                case.name
            );
            for node in [text[0], text[2]] {
                assert!(
                    node.ancestors()
                        .all(|ancestor| ancestor.attribute("clip-path").is_none()),
                    "{}",
                    case.name
                );
            }
            assert_eq!(
                text[1]
                    .ancestors()
                    .any(|ancestor| ancestor.attribute("clip-path").is_some()),
                case.backend_clip,
                "{}",
                case.name
            );
            let rectangle = xml.descendants().find(|node| node.has_tag_name("rect"));
            assert_eq!(rectangle.is_some(), case.backend_clip, "{}", case.name);
            if let Some(rectangle) = rectangle {
                let [left, top, right, bottom] = case.world_clip.unwrap().map(f64::from);
                for (attribute, expected) in [
                    ("x", left + shift[0]),
                    ("y", top + shift[1]),
                    ("width", right - left),
                    ("height", bottom - top),
                ] {
                    assert_eq!(
                        rectangle
                            .attribute(attribute)
                            .unwrap()
                            .parse::<f64>()
                            .unwrap(),
                        expected,
                        "{} {attribute}",
                        case.name
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_output_offsets_fail_before_any_text_scope_is_painted() {
        let clip = NativeCellRunClip::Clipped {
            world_rect: [0.0, 0.0, 10.0, 10.0],
            intersects_source: false,
        };
        for offset in [f64::NAN, f64::INFINITY, f64::MAX] {
            assert!(NativeRunClip::new(clip, [offset; 2]).is_err());
        }
    }
}
