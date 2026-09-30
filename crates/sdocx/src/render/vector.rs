use base64::Engine as _;
use std::{fmt, marker::PhantomData};
use svg::{
    Node as SvgNode,
    node::{Value, element as svg_element},
};

mod path;
mod values;
pub(super) use path::{Data, ReplayPath, coordinate, polyline};
pub(super) use values::*;

pub(super) struct Node {
    element: svg_element::Element,
    valid: bool,
}
impl Node {
    fn new(element: impl Into<svg_element::Element>) -> Self {
        Self {
            element: element.into(),
            valid: true,
        }
    }
    fn attr(&mut self, name: &str, value: impl Into<Value>) {
        self.element.assign(name, value);
    }
    fn style(&mut self, name: &str, value: &str) {
        let mut declarations = self
            .element
            .get_attributes()
            .get("style")
            .map(|style| {
                style
                    .split(';')
                    .filter(|declaration| {
                        declaration
                            .split_once(':')
                            .is_some_and(|(property, _)| property != name)
                    })
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        declarations.push(format!("{name}:{value}"));
        self.attr("style", declarations.join(";"));
    }
    fn optional(&mut self, name: &str, value: Option<String>) {
        if let Some(value) = value {
            self.attr(name, value);
        } else {
            self.valid = false;
        }
    }
    fn number(
        &mut self,
        name: &str,
        value: impl Into<Numeric>,
        range: std::ops::RangeInclusive<f64>,
    ) {
        let value = value
            .into()
            .0
            .filter(|value| range.contains(&value.value()))
            .map(Number::text);
        self.optional(name, value);
    }
    fn append(&mut self, child: Node) {
        if child.valid {
            self.element.append(Inline(child.element));
        }
    }
}

pub(super) trait NodeAccess {
    fn node(&mut self) -> &mut Node;
}
pub(super) trait Container: Into<Node> {}
macro_rules! elements {
    ($($name:ident),*) => { $(
        pub(super) struct $name(Node);
        impl From<$name> for Node { fn from(value: $name) -> Self { value.0 } }
        impl NodeAccess for $name { fn node(&mut self) -> &mut Node { &mut self.0 } }
    )* };
}
elements!(
    Svg,
    Group,
    Definitions,
    ClipPath,
    LinearGradient,
    SvgMask,
    Stop,
    Path,
    Circle,
    Ellipse,
    Rectangle,
    Line,
    Polygon,
    Polyline,
    Image,
    Text,
    TSpan,
    Anchor,
    EmbeddedFont
);
macro_rules! constructors {
    ($($name:ident),*) => { $(impl $name { pub fn new() -> Self { Self(Node::new(svg_element::$name::new())) } })* };
}
constructors!(
    Group,
    Definitions,
    Path,
    Circle,
    Ellipse,
    Rectangle,
    Line,
    Polygon,
    Polyline
);
macro_rules! containers { ($($name:ident),*) => { $(impl Container for $name {})* }; }
containers!(
    Svg,
    Group,
    Definitions,
    ClipPath,
    LinearGradient,
    SvgMask,
    Text,
    Anchor
);
macro_rules! children {
    ($($name:ident),*) => { $(impl $name {
        pub fn add(mut self, child: impl Into<Node>) -> Self { self.0.append(child.into()); self }
    })* };
}
children!(Group, Definitions, ClipPath, LinearGradient);

pub(super) struct Scene {
    elements: Vec<Node>,
    next_id: usize,
}
impl Scene {
    pub fn new(root: Svg) -> Self {
        Self {
            elements: vec![root.into()],
            next_id: 0,
        }
    }
    pub fn push(&mut self, node: impl Into<Node>) {
        let node = node.into();
        if !node.valid {
            return;
        }
        if self.elements.len() == 1 {
            // Only serialized nodes enter Blob; release completed subtrees without retaining a page DOM.
            self.elements[0]
                .element
                .append(svg::node::Blob::new(node.element.to_string()));
        } else {
            self.elements.last_mut().unwrap().append(node);
        }
    }
    pub fn scope(&mut self, element: impl Container, draw: impl FnOnce(&mut Self)) {
        let element = element.into();
        if !element.valid {
            return;
        }
        self.elements.push(element);
        draw(self);
        let element = self.elements.pop().unwrap();
        self.push(element);
    }
    pub fn definition<K>(&mut self) -> Definition<K> {
        let id = self.next_id;
        self.next_id += 1;
        Definition {
            id,
            kind: PhantomData,
        }
    }
    pub fn finish(self) -> String {
        assert_eq!(self.elements.len(), 1);
        if self.elements[0].valid {
            self.elements[0].element.to_string()
        } else {
            svg_element::SVG::new().to_string()
        }
    }
}

// Svg formatting whitespace must not enter preserved rich text and links.
#[derive(Clone, Debug)]
struct Inline(svg_element::Element);
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
impl SvgNode for Inline {
    fn get_name(&self) -> &str {
        self.0.get_name()
    }
    fn is_bare(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy)]
pub(super) enum Gradient {}
#[derive(Clone, Copy)]
pub(super) enum Mask {}
#[derive(Clone, Copy)]
pub(super) enum Clip {}
#[derive(Clone, Copy)]
pub(super) struct Definition<K> {
    id: usize,
    kind: PhantomData<K>,
}
impl<K> Definition<K> {
    fn id(&self) -> String {
        format!("sdocx-def-{}", self.id)
    }
    fn url(&self) -> String {
        format!("url(#{})", self.id())
    }
}

pub(super) trait Styled: NodeAccess + Sized {
    fn fill(mut self, paint: impl Into<Option<Paint>>) -> Self {
        self.node().optional("fill", paint.into().map(Paint::text));
        self
    }
    fn stroke(mut self, paint: impl Into<Option<Paint>>) -> Self {
        self.node()
            .optional("stroke", paint.into().map(Paint::text));
        self
    }
    fn fill_opacity(mut self, value: impl Into<Numeric>) -> Self {
        self.node().number("fill-opacity", value, 0.0..=1.0);
        self
    }
    fn stroke_opacity(mut self, value: impl Into<Numeric>) -> Self {
        self.node().number("stroke-opacity", value, 0.0..=1.0);
        self
    }
    fn stroke_width(mut self, value: impl Into<Numeric>) -> Self {
        self.node().number("stroke-width", value, 0.0..=f64::MAX);
        self
    }
    fn line_cap(mut self, cap: LineCap) -> Self {
        self.node().attr("stroke-linecap", cap.text());
        self
    }
    fn line_join(mut self, join: LineJoin) -> Self {
        self.node().attr("stroke-linejoin", join.text());
        self
    }
    fn transformed(mut self, transform: Transform) -> Self {
        self.node().optional("transform", transform.text());
        self
    }
    fn masked(mut self, id: &Definition<Mask>) -> Self {
        self.node().attr("mask", id.url());
        self
    }
    fn clipped(mut self, id: &Definition<Clip>) -> Self {
        self.node().attr("clip-path", id.url());
        self
    }
    fn replay_part(mut self, part: Option<ReplayPart>) -> Self {
        if let Some(ReplayPart(part)) = part {
            self.node().valid &= part > 0;
            self.node().attr("data-replay-part", part);
        }
        self
    }
}
macro_rules! styles { ($($name:ident),*) => { $(impl Styled for $name {})* }; }
styles!(
    Svg, Group, Path, Circle, Ellipse, Rectangle, Line, Polygon, Polyline, Image, Text, TSpan,
    Anchor
);

macro_rules! numeric_attributes {
    ($($ty:ident { $($method:ident => ($name:literal, $min:expr)),* $(,)? })*) => { $(impl $ty { $(
        pub fn $method(mut self, value: impl Into<Numeric>) -> Self {
            self.0.number($name, value, $min..=f64::MAX); self
        }
    )* })* };
}
numeric_attributes! {
    Svg { x => ("x", -f64::MAX), y => ("y", -f64::MAX), width => ("width", 0.), height => ("height", 0.) }
    Rectangle { x => ("x", -f64::MAX), y => ("y", -f64::MAX), width => ("width", 0.), height => ("height", 0.), rx => ("rx", 0.) }
    Circle { cx => ("cx", -f64::MAX), cy => ("cy", -f64::MAX), r => ("r", 0.) }
    Ellipse { cx => ("cx", -f64::MAX), cy => ("cy", -f64::MAX), rx => ("rx", 0.), ry => ("ry", 0.) }
    Line { x1 => ("x1", -f64::MAX), y1 => ("y1", -f64::MAX), x2 => ("x2", -f64::MAX), y2 => ("y2", -f64::MAX) }
    Image { x => ("x", -f64::MAX), y => ("y", -f64::MAX), width => ("width", 0.), height => ("height", 0.) }
    Text { x => ("x", -f64::MAX), y => ("y", -f64::MAX), font_size => ("font-size", 0.) }
    TSpan { font_size => ("font-size", 0.) }
    SvgMask { x => ("x", -f64::MAX), y => ("y", -f64::MAX), width => ("width", 0.), height => ("height", 0.) }
    LinearGradient { x1 => ("x1", -f64::MAX), y1 => ("y1", -f64::MAX), x2 => ("x2", -f64::MAX), y2 => ("y2", -f64::MAX) }
}
impl Svg {
    pub fn new() -> Self {
        Self(Node::new(svg_element::SVG::new()))
    }
    pub fn view_box(mut self, bounds: ViewBox) -> Self {
        self.0.optional("viewBox", bounds.text());
        self
    }
    pub fn clipped_viewport(mut self) -> Self {
        self.0.attr("overflow", "hidden");
        self
    }
}
impl Group {
    pub fn blend(mut self, mode: Blend) -> Self {
        self.0.style("mix-blend-mode", mode.text());
        self
    }
    pub fn isolated(mut self) -> Self {
        self.0.style("isolation", "isolate");
        self
    }
    pub fn flow(mut self) -> Self {
        self.0.attr("data-sdocx-flow", "true");
        self
    }
    pub fn object(mut self, kind: ObjectKind) -> Self {
        self.0.attr("data-sdocx-object", kind.text());
        self
    }
    pub fn replay_stroke(mut self, index: StrokeIndex) -> Self {
        self.0.attr("data-replay-stroke", index.0);
        self
    }
}
impl Path {
    pub fn template(mut self, kind: PageTemplate) -> Self {
        self.0.attr("data-page-template", kind.text());
        self
    }
    pub fn dotted(mut self, pitch: impl Into<Numeric>) -> Self {
        let pitch = pitch
            .into()
            .0
            .filter(|n| n.value() > 0.)
            .map(|n| svg::node::Value::from((0, n.text())).to_string());
        self.0.optional("stroke-dasharray", pitch);
        self
    }
}
impl Polygon {
    pub fn points(mut self, points: &[(f64, f64)], places: usize) -> Self {
        let pairs = points
            .iter()
            .map(|(x, y)| Some((decimal(*x, places)?.text(), decimal(*y, places)?.text())))
            .collect::<Option<Vec<_>>>();
        self.0
            .optional("points", pairs.map(|pairs| Value::from(pairs).to_string()));
        self
    }
}
impl Image {
    pub fn embedded(data: &[u8], mime: &str) -> Self {
        let mut node = Node::new(svg_element::Image::new());
        node.attr(
            "href",
            format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(data)
            ),
        );
        Self(node)
    }
    pub fn stretched(mut self) -> Self {
        self.0.attr("preserveAspectRatio", "none");
        self
    }
}
impl Text {
    pub fn new(content: &str) -> Self {
        Self(Node::new(svg_element::Text::new(content)))
    }
    pub fn family(mut self, family: FontFamily<'_>) -> Self {
        self.0.attr("font-family", family.text());
        self
    }
    pub fn anchor(mut self, anchor: TextAnchor) -> Self {
        self.0.attr("text-anchor", anchor.text());
        self
    }
    pub fn preserve_space(mut self) -> Self {
        self.0.attr("xml:space", "preserve");
        self
    }
}
impl TSpan {
    pub fn new(content: &str) -> Self {
        Self(Node::new(svg_element::TSpan::new(content)))
    }
    pub fn family(mut self, family: FontFamily<'_>) -> Self {
        self.0.attr("font-family", family.text());
        self
    }
    pub fn bold(mut self) -> Self {
        self.0.attr("font-weight", "bold");
        self
    }
    pub fn italic(mut self) -> Self {
        self.0.attr("font-style", "italic");
        self
    }
    pub fn decoration(mut self, decoration: TextDecoration) -> Self {
        self.0.attr("text-decoration", decoration.text());
        self
    }
    pub fn stroke_under_fill(mut self) -> Self {
        self.0.attr("paint-order", "stroke fill");
        self
    }
}

impl EmbeddedFont {
    pub fn new(family: &str, weight: u16, style: fontdb::Style, data: &[u8]) -> Self {
        let mut name = String::new();
        cssparser::serialize_string(family, &mut name).expect("writing a CSS string");
        let bytes = base64::engine::general_purpose::STANDARD.encode(data);
        let slant = match style {
            fontdb::Style::Normal => "normal",
            fontdb::Style::Italic => "italic",
            fontdb::Style::Oblique => "oblique",
        };
        let css = format!(
            "@font-face{{font-family:{name};font-weight:{weight};font-style:{slant};src:url(\"data:font/ttf;base64,{bytes}\") format(\"truetype\")}}"
        );
        Self(Node::new(svg_element::Style::new(css)))
    }
}
impl Anchor {
    pub fn new(target: &str) -> Self {
        Self(Node::new(svg_element::Anchor::new().set("href", target)))
    }
}
impl ClipPath {
    pub fn new(id: &Definition<Clip>) -> Self {
        Self(Node::new(svg_element::ClipPath::new().set("id", id.id())))
    }
}
impl LinearGradient {
    pub fn new(id: &Definition<Gradient>) -> Self {
        Self(Node::new(
            svg_element::LinearGradient::new()
                .set("id", id.id())
                .set("gradientUnits", "userSpaceOnUse"),
        ))
    }
}
impl SvgMask {
    pub fn luminance(id: &Definition<Mask>) -> Self {
        Self(Node::new(
            svg_element::Mask::new()
                .set("id", id.id())
                .set("maskUnits", "userSpaceOnUse")
                .set("style", "mask-type:luminance"),
        ))
    }
}
impl Stop {
    pub fn new(offset: impl Into<Numeric>, color: impl Into<Option<ColorValue>>) -> Self {
        let mut node = Node::new(svg_element::Stop::new());
        node.number("offset", offset, 0.0..=1.0);
        node.optional("stop-color", color.into().map(ColorValue::text));
        Self(node)
    }
}

#[cfg(test)]
mod tests;
