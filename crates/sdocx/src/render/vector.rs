use std::fmt;
use svg::Node;
pub(super) use svg::node::element::path::Data;
use svg::node::element::{Element, Group, Path, Polyline};

pub(super) struct Scene {
    elements: Vec<Element>,
    next_id: usize,
}

impl Scene {
    pub fn new(root: impl Into<Element>) -> Self {
        Self {
            elements: vec![root.into()],
            next_id: 0,
        }
    }

    pub fn push(&mut self, node: impl Node) {
        if self.elements.len() == 1 {
            // Only library-serialized nodes enter Blob; never unescaped source text.
            // Release completed subtrees instead of retaining a page-sized DOM.
            self.elements[0].append(svg::node::Blob::new(node.to_string()));
        } else {
            self.elements.last_mut().unwrap().append(node);
        }
    }

    pub fn scope(&mut self, element: impl Into<Element>, draw: impl FnOnce(&mut Self)) {
        self.elements.push(element.into());
        draw(self);
        let element = self.elements.pop().unwrap();
        self.push(Inline(element));
    }

    pub fn definition<K>(&mut self) -> Definition<K> {
        let id = self.next_id;
        self.next_id += 1;
        Definition {
            id,
            kind: std::marker::PhantomData,
        }
    }

    pub fn finish(self) -> String {
        assert_eq!(self.elements.len(), 1);
        self.elements[0].to_string()
    }
}

// Keep library-added formatting whitespace out of text with xml:space="preserve".
#[derive(Clone, Debug)]
pub(super) struct Inline(pub Element);

impl fmt::Display for Inline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl svg::node::NodeDefaultHash for Inline {
    fn default_hash(&self, state: &mut std::collections::hash_map::DefaultHasher) {
        self.0.default_hash(state);
    }
}

impl Node for Inline {
    fn get_name(&self) -> &str {
        self.0.get_name()
    }
    fn is_bare(&self) -> bool {
        true
    }
}

pub(super) enum Gradient {}
pub(super) enum Mask {}
pub(super) enum Clip {}

pub(super) struct Definition<K> {
    id: usize,
    kind: std::marker::PhantomData<K>,
}

impl<K> Definition<K> {
    pub fn id(&self) -> String {
        format!("sdocx-def-{}", self.id)
    }
    fn url(&self) -> String {
        format!("url(#{})", self.id())
    }
}

pub(super) enum Paint<'a> {
    None,
    Color(&'a str),
    Gradient(&'a Definition<Gradient>),
}

pub(super) enum Blend {
    Darken,
    Lighten,
}

pub(super) trait Styled: Node + Sized {
    fn fill(mut self, paint: Paint<'_>) -> Self {
        self.assign(
            "fill",
            match paint {
                Paint::None => "none".to_owned(),
                Paint::Color(color) => color.to_owned(),
                Paint::Gradient(id) => id.url(),
            },
        );
        self
    }

    fn masked(mut self, id: &Definition<Mask>) -> Self {
        self.assign("mask", id.url());
        self
    }

    fn clipped(mut self, id: &Definition<Clip>) -> Self {
        self.assign("clip-path", id.url());
        self
    }

    fn replay_part(mut self, part: Option<usize>) -> Self {
        if let Some(part) = part {
            self.assign("data-replay-part", part);
        }
        self
    }
}
impl<T: Node> Styled for T {}

pub(super) fn blend(mode: Blend) -> Group {
    Group::new().set(
        "style",
        match mode {
            Blend::Darken => "mix-blend-mode:darken",
            Blend::Lighten => "mix-blend-mode:lighten",
        },
    )
}

#[derive(Default)]
pub(super) struct ReplayPath {
    data: String,
    lengths: Vec<usize>,
}

impl ReplayPath {
    pub fn push(&mut self, part: Data, replay: bool) {
        self.data.push_str(&svg::node::Value::from(part));
        if replay {
            self.lengths.push(self.data.len());
        }
    }

    pub fn finish(self) -> Path {
        let mut path = Path::new().set("d", self.data);
        if !self.lengths.is_empty() {
            path.assign("data-replay-lengths", lengths(&self.lengths));
        }
        path
    }
}

pub(super) fn polyline(points: &[crate::Point], replay: bool) -> Polyline {
    let mut value = String::new();
    let mut offsets = Vec::new();
    for point in points {
        if !value.is_empty() {
            value.push(' ');
        }
        value.push_str(&svg::node::Value::from((
            decimal(point.x, 2),
            decimal(point.y, 2),
        )));
        if replay {
            offsets.push(value.len());
        }
    }
    let mut node = Polyline::new().set("points", value);
    if replay && !offsets.is_empty() {
        node.assign("data-replay-lengths", lengths(&offsets));
    }
    node
}

pub(super) fn lengths(values: &[usize]) -> String {
    values
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}")
}

pub(super) fn coordinate(value: f64, places: usize) -> f32 {
    // Preserve the existing export's decimal rounding before the library's f32 boundary.
    decimal(value, places).parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use svg::node::element::{Anchor, SVG, TSpan, Text};

    #[test]
    fn rich_text_preserves_whitespace_and_escapes_content_and_links() {
        let mut scene = Scene::new(SVG::new());
        let label = "  A&B <text> \"quoted\"\n🙂 ";
        let target = "https://example.test/?a=1&b=\"two\"";
        scene.scope(Text::new("").set("xml:space", "preserve"), |scene| {
            scene.push(Inline(TSpan::new(label).into()));
            scene.scope(Anchor::new().set("href", target), |scene| {
                scene.push(Inline(TSpan::new("linked text").into()));
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
                    .elliptical_arc_by((3, 3, 0, 1, 0, 6, 0))
                    .elliptical_arc_by((3, 3, 0, 1, 0, -6, 0))
                    .close(),
                true,
            );
        }
        let output = SVG::new().add(path.finish()).to_string();
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
        let mut scene = Scene::new(SVG::new());
        let gradient = scene.definition::<Gradient>();
        scene.scope(Group::new(), |scene| {
            let mask = scene.definition::<Mask>();
            assert_ne!(gradient.id(), mask.id());
        });
        let clip = scene.definition::<Clip>();
        assert_eq!(clip.id(), "sdocx-def-2");
        assert_eq!(
            Scene::new(SVG::new()).definition::<Mask>().id(),
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
        let output = SVG::new().add(polyline(&points, true)).to_string();
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
                assert_eq!(*x, decimal(expected.x, 2).parse::<f64>().unwrap());
                assert_eq!(*y, decimal(expected.y, 2).parse::<f64>().unwrap());
            }
        }
    }
}
