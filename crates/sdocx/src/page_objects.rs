use crate::{ObjectRenderLayer, Page, PageElement, Stroke};

/// An object at its stored position within a page or container.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageObject {
    pub render_layer: ObjectRenderLayer,
    pub source_offset: Option<usize>,
    pub content: PageObjectContent,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PageObjectContent {
    Stroke(Stroke),
    Element(PageElement),
    Container(Vec<PageObject>),
    /// Content promoted to the page root after flattening a non-container parent.
    Descendant(Box<PageObjectContent>),
}

impl From<Stroke> for PageObject {
    fn from(stroke: Stroke) -> Self {
        Self {
            render_layer: ObjectRenderLayer::Base,
            source_offset: None,
            content: PageObjectContent::Stroke(stroke),
        }
    }
}

impl From<PageElement> for PageObject {
    fn from(element: PageElement) -> Self {
        Self {
            render_layer: ObjectRenderLayer::Base,
            source_offset: None,
            content: PageObjectContent::Element(element),
        }
    }
}

impl Page {
    pub fn strokes(&self) -> impl Iterator<Item = &Stroke> {
        ObjectIter::new(&self.objects).filter_map(|object| match object.content.leaf() {
            PageObjectContent::Stroke(stroke) => Some(stroke),
            _ => None,
        })
    }

    pub fn strokes_mut(&mut self) -> impl Iterator<Item = &mut Stroke> {
        ContentIterMut::new(&mut self.objects).filter_map(|content| match content {
            PageObjectContent::Stroke(stroke) => Some(stroke),
            _ => None,
        })
    }

    pub fn elements(&self) -> impl Iterator<Item = &PageElement> {
        ObjectIter::new(&self.objects).filter_map(|object| match object.content.leaf() {
            PageObjectContent::Element(element) => Some(element),
            _ => None,
        })
    }

    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut PageElement> {
        ContentIterMut::new(&mut self.objects).filter_map(|content| match content {
            PageObjectContent::Element(element) => Some(element),
            _ => None,
        })
    }

    pub fn clear_strokes(&mut self) {
        retain_content(&mut self.objects, |content| {
            !matches!(content, PageObjectContent::Stroke(_))
        });
    }

    pub fn clear_elements(&mut self) {
        retain_content(&mut self.objects, |content| {
            !matches!(content, PageObjectContent::Element(_))
        });
    }
}

#[cfg(feature = "render")]
impl Page {
    /// Accepted objects in paint order, including selected container parents.
    pub fn composed_objects(&self) -> impl Iterator<Item = &PageObject> {
        use crate::composition::RenderPass;
        [RenderPass::Base, RenderPass::Top, RenderPass::Masking]
            .into_iter()
            .flat_map(move |pass| {
                self.objects
                    .iter()
                    .filter(move |root| root.render_pass() == Some(pass))
                    .flat_map(|root| ObjectIter::new(std::slice::from_ref(root)))
            })
    }

    /// Accepted objects in paint order with inherited native-geometry
    /// admission, including container parents and non-stroke objects. This
    /// permits callers to interleave their own handling of mixed page content.
    pub fn composed_objects_with_context(&self) -> Vec<(&PageObject, bool)> {
        fn visit<'a>(object: &'a PageObject, allowed: bool, out: &mut Vec<(&'a PageObject, bool)>) {
            let allowed = allowed && !matches!(object.content, PageObjectContent::Descendant(_));
            out.push((object, allowed));
            fn children<'a>(
                content: &'a PageObjectContent,
                allowed: bool,
                out: &mut Vec<(&'a PageObject, bool)>,
            ) {
                match content {
                    PageObjectContent::Container(objects) => {
                        for object in objects {
                            visit(object, allowed, out);
                        }
                    }
                    PageObjectContent::Descendant(inner) => children(inner, false, out),
                    _ => {}
                }
            }
            children(&object.content, allowed, out);
        }

        use crate::composition::RenderPass;
        let mut objects = Vec::new();
        for pass in [RenderPass::Base, RenderPass::Top, RenderPass::Masking] {
            for root in self
                .objects
                .iter()
                .filter(|root| root.render_pass() == Some(pass))
            {
                visit(root, true, &mut objects);
            }
        }
        objects
    }

    /// Strokes accepted by root composition, in stored traversal order.
    pub fn composed_strokes(&self) -> impl Iterator<Item = (&PageObject, &Stroke)> {
        self.objects
            .iter()
            .filter(|root| root.render_pass().is_some())
            .flat_map(|root| ObjectIter::new(std::slice::from_ref(root)))
            .filter_map(|object| match object.content.leaf() {
                PageObjectContent::Stroke(stroke) => Some((object, stroke)),
                _ => None,
            })
    }

    /// Strokes accepted by root composition with the renderer's inherited
    /// native-geometry admission state. A promoted descendant disables native
    /// geometry for its entire subtree, including descendants inside a
    /// container.
    pub fn composed_strokes_with_context(&self) -> Vec<(&PageObject, &Stroke, bool)> {
        self.contextual_strokes(false)
    }

    /// Accepted strokes in composition paint order, with the renderer's
    /// inherited native-geometry admission state. This traversal omits
    /// non-stroke objects; use `composed_objects_with_context` when those objects must be
    /// interleaved with strokes. Its indices differ from stored traversal order.
    pub fn composed_strokes_in_paint_order_with_context(
        &self,
    ) -> Vec<(&PageObject, &Stroke, bool)> {
        self.contextual_strokes(true)
    }

    fn contextual_strokes(&self, paint_order: bool) -> Vec<(&PageObject, &Stroke, bool)> {
        fn visit<'a>(
            object: &'a PageObject,
            native_allowed: bool,
            out: &mut Vec<(&'a PageObject, &'a Stroke, bool)>,
        ) {
            let native_allowed =
                native_allowed && !matches!(object.content, PageObjectContent::Descendant(_));
            visit_content(object, &object.content, native_allowed, out);
        }

        fn visit_content<'a>(
            object: &'a PageObject,
            content: &'a PageObjectContent,
            native_allowed: bool,
            out: &mut Vec<(&'a PageObject, &'a Stroke, bool)>,
        ) {
            match content {
                PageObjectContent::Stroke(stroke) => out.push((object, stroke, native_allowed)),
                PageObjectContent::Container(children) => {
                    for child in children {
                        visit(child, native_allowed, out);
                    }
                }
                PageObjectContent::Descendant(inner) => {
                    visit_content(object, inner, false, out);
                }
                PageObjectContent::Element(_) => {}
            }
        }

        let mut strokes = Vec::new();
        if paint_order {
            use crate::composition::RenderPass;
            for pass in [RenderPass::Base, RenderPass::Top, RenderPass::Masking] {
                for root in self
                    .objects
                    .iter()
                    .filter(|root| root.render_pass() == Some(pass))
                {
                    visit(root, true, &mut strokes);
                }
            }
        } else {
            for root in self
                .objects
                .iter()
                .filter(|root| root.render_pass().is_some())
            {
                visit(root, true, &mut strokes);
            }
        }
        strokes
    }
}

#[cfg(feature = "render")]
impl PageObject {
    pub(crate) fn stroke_count(&self) -> usize {
        ObjectIter::new(std::slice::from_ref(self))
            .filter(|object| matches!(object.content.leaf(), PageObjectContent::Stroke(_)))
            .count()
    }
}

fn retain_content(
    objects: &mut Vec<PageObject>,
    retain: impl Copy + Fn(&PageObjectContent) -> bool,
) {
    objects.retain_mut(|object| match &mut object.content {
        PageObjectContent::Container(children) => {
            retain_content(children, retain);
            true
        }
        PageObjectContent::Descendant(content) => retain_descendant(content, retain),
        content => retain(content),
    });
}

fn retain_descendant(
    content: &mut PageObjectContent,
    retain: impl Copy + Fn(&PageObjectContent) -> bool,
) -> bool {
    match content {
        PageObjectContent::Container(children) => {
            retain_content(children, retain);
            true
        }
        PageObjectContent::Descendant(inner) => retain_descendant(inner, retain),
        content => retain(content),
    }
}

impl PageObjectContent {
    /// Return the stroke held by this object, including a promoted descendant.
    /// Container children are visited separately by page object iterators.
    pub fn as_stroke(&self) -> Option<&Stroke> {
        match self.leaf() {
            Self::Stroke(stroke) => Some(stroke),
            _ => None,
        }
    }

    pub(crate) fn leaf(&self) -> &Self {
        match self {
            Self::Descendant(content) => content.leaf(),
            content => content,
        }
    }

    fn leaf_mut(&mut self) -> &mut Self {
        match self {
            Self::Descendant(content) => content.leaf_mut(),
            content => content,
        }
    }

    fn children(&self) -> Option<&[PageObject]> {
        match self {
            Self::Container(children) => Some(children),
            Self::Descendant(content) => content.children(),
            _ => None,
        }
    }
}

struct ObjectIter<'a> {
    stack: Vec<std::slice::Iter<'a, PageObject>>,
}

impl<'a> ObjectIter<'a> {
    fn new(objects: &'a [PageObject]) -> Self {
        Self {
            stack: vec![objects.iter()],
        }
    }
}

impl<'a> Iterator for ObjectIter<'a> {
    type Item = &'a PageObject;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(objects) = self.stack.last_mut() {
            if let Some(object) = objects.next() {
                if let Some(children) = object.content.children() {
                    self.stack.push(children.iter());
                }
                return Some(object);
            }
            self.stack.pop();
        }
        None
    }
}

struct ContentIterMut<'a> {
    stack: Vec<std::slice::IterMut<'a, PageObject>>,
}

impl<'a> ContentIterMut<'a> {
    fn new(objects: &'a mut [PageObject]) -> Self {
        Self {
            stack: vec![objects.iter_mut()],
        }
    }
}

impl<'a> Iterator for ContentIterMut<'a> {
    type Item = &'a mut PageObjectContent;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(objects) = self.stack.last_mut() {
            if let Some(object) = objects.next() {
                match object.content.leaf_mut() {
                    PageObjectContent::Container(children) => self.stack.push(children.iter_mut()),
                    content => return Some(content),
                }
            } else {
                self.stack.pop();
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_with_flattened_image() -> Page {
        let child = PageObject {
            render_layer: ObjectRenderLayer::Base,
            source_offset: Some(12),
            content: PageObjectContent::Element(PageElement::Image {
                bbox: crate::BoundingBox::default(),
                media_index: 3,
            }),
        };
        Page {
            uuid: "flattened-child".into(),
            width: 100,
            height: 100,
            content_bbox: crate::BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageObject {
                render_layer: child.render_layer,
                source_offset: child.source_offset,
                content: PageObjectContent::Descendant(Box::new(child.content)),
            }],
        }
    }

    fn empty_stroke() -> Stroke {
        Stroke {
            rendering: None,
            bbox: crate::BoundingBox::default(),
            points: Vec::new(),
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: 1.0,
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn composed_stroke_context_inherits_promoted_descendant_admission() {
        let make_stroke = |x| {
            let mut stroke = empty_stroke();
            stroke.points.push(crate::Point { x, y: 0.0 });
            stroke
        };
        let object = |content| PageObject {
            render_layer: ObjectRenderLayer::Base,
            source_offset: None,
            content,
        };
        let page = Page {
            uuid: "context-provenance".into(),
            width: 100,
            height: 100,
            content_bbox: crate::BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![object(PageObjectContent::Container(vec![
                object(PageObjectContent::Stroke(make_stroke(1.0))),
                object(PageObjectContent::Descendant(Box::new(
                    PageObjectContent::Container(vec![object(PageObjectContent::Stroke(
                        make_stroke(2.0),
                    ))]),
                ))),
            ]))],
        };
        let contextual = page.composed_strokes_with_context();
        assert_eq!(
            contextual
                .iter()
                .map(|(_, stroke, native_allowed)| { (stroke.points[0].x, *native_allowed) })
                .collect::<Vec<_>>(),
            [(1.0, true), (2.0, false)]
        );
        assert_eq!(
            page.composed_objects_with_context()
                .into_iter()
                .filter_map(|(object, allowed)| object
                    .content
                    .as_stroke()
                    .map(|stroke| (stroke.points[0].x, allowed)))
                .collect::<Vec<_>>(),
            [(1.0, true), (2.0, false)]
        );

        let mut mixed = page;
        mixed.objects = [
            (ObjectRenderLayer::Top, 3.0),
            (ObjectRenderLayer::Base, 1.0),
            (ObjectRenderLayer::Masking, 2.0),
        ]
        .into_iter()
        .map(|(render_layer, x)| PageObject {
            render_layer,
            source_offset: None,
            content: PageObjectContent::Stroke(make_stroke(x)),
        })
        .collect();
        mixed
            .objects
            .push(object(PageObjectContent::Descendant(Box::new(
                PageObjectContent::Container(vec![object(PageObjectContent::Stroke(make_stroke(
                    4.0,
                )))]),
            ))));
        mixed.objects.push(PageObject {
            render_layer: ObjectRenderLayer::Other(9),
            source_offset: None,
            content: PageObjectContent::Stroke(make_stroke(5.0)),
        });
        assert_eq!(
            mixed
                .composed_objects()
                .filter_map(|object| object.content.as_stroke())
                .map(|stroke| stroke.points[0].x)
                .collect::<Vec<_>>(),
            [1.0, 4.0, 3.0, 2.0]
        );
        assert_eq!(
            mixed
                .composed_strokes_with_context()
                .iter()
                .map(|(_, stroke, _)| stroke.points[0].x)
                .collect::<Vec<_>>(),
            mixed
                .composed_strokes()
                .map(|(_, stroke)| stroke.points[0].x)
                .collect::<Vec<_>>(),
            "contextual metadata must retain stored-order replay stroke indices"
        );
        assert_eq!(
            mixed
                .composed_strokes_in_paint_order_with_context()
                .iter()
                .map(|(_, stroke, native_allowed)| (stroke.points[0].x, *native_allowed))
                .collect::<Vec<_>>(),
            [(1.0, true), (4.0, false), (3.0, true), (2.0, true)]
        );
        assert_eq!(
            mixed
                .composed_objects_with_context()
                .into_iter()
                .filter_map(|(object, allowed)| object
                    .content
                    .as_stroke()
                    .map(|stroke| (stroke.points[0].x, allowed)))
                .collect::<Vec<_>>(),
            [(1.0, true), (4.0, false), (3.0, true), (2.0, true)]
        );
    }

    #[test]
    fn flattened_descendants_remain_visible_to_page_element_apis() {
        let mut page = page_with_flattened_image();
        assert!(matches!(
            page.elements().next(),
            Some(PageElement::Image { media_index: 3, .. })
        ));
        let PageElement::Image { bbox, .. } = page.elements_mut().next().unwrap() else {
            panic!("flattened image was hidden from mutable traversal");
        };
        bbox.x_min = 8.0;
        let PageElement::Image { bbox, .. } = page.elements().next().unwrap() else {
            panic!("flattened element changed kind");
        };
        assert_eq!(bbox.x_min, 8.0);

        page.clear_elements();
        assert_eq!(page.elements().count(), 0);
        assert!(page.objects.is_empty());
    }

    #[test]
    fn nested_flattened_descendants_remain_visible_to_mutable_traversal() {
        let mut page = page_with_flattened_image();
        page.objects[0].content = PageObjectContent::Descendant(Box::new(
            PageObjectContent::Descendant(Box::new(page.objects[0].content.clone())),
        ));
        page.objects.push(PageObject {
            render_layer: ObjectRenderLayer::Base,
            source_offset: Some(20),
            content: PageObjectContent::Descendant(Box::new(PageObjectContent::Descendant(
                Box::new(PageObjectContent::Container(vec![PageObject {
                    render_layer: ObjectRenderLayer::Base,
                    source_offset: Some(21),
                    content: PageObjectContent::Descendant(Box::new(PageObjectContent::Stroke(
                        empty_stroke(),
                    ))),
                }])),
            ))),
        });

        assert_eq!(page.elements().count(), 1);
        assert_eq!(page.strokes().count(), 1);
        assert!(page.objects[1].content.as_stroke().is_none());
        let PageObjectContent::Descendant(container) = &page.objects[1].content else {
            unreachable!()
        };
        let PageObjectContent::Descendant(container) = &**container else {
            unreachable!()
        };
        let PageObjectContent::Container(children) = &**container else {
            unreachable!()
        };
        assert!(children[0].content.as_stroke().is_some());
        #[cfg(feature = "render")]
        assert_eq!(page.objects[1].stroke_count(), 1);
        page.strokes_mut().next().unwrap().pen_width = 4.0;
        assert_eq!(page.strokes().next().unwrap().pen_width, 4.0);

        page.clear_elements();
        assert_eq!(page.elements().count(), 0);
        assert_eq!(page.strokes().count(), 1);
        page.clear_strokes();
        assert_eq!(page.strokes().count(), 0);
    }
}
