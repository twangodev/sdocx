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
