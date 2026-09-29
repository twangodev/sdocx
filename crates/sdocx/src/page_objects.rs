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
        ObjectIter::new(&self.objects).filter_map(|object| match &object.content {
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
        ObjectIter::new(&self.objects).filter_map(|object| match &object.content {
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

fn retain_content(
    objects: &mut Vec<PageObject>,
    retain: impl Copy + Fn(&PageObjectContent) -> bool,
) {
    objects.retain_mut(|object| match &mut object.content {
        PageObjectContent::Container(children) => {
            retain_content(children, retain);
            true
        }
        content => retain(content),
    });
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
                if let PageObjectContent::Container(children) = &object.content {
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
                match &mut object.content {
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
