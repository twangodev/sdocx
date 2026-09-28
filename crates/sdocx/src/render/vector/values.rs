use super::{Definition, Gradient};

#[derive(Clone, Copy, Debug)]
pub struct Number {
    value: f64,
    places: Option<usize>,
}

impl Number {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self {
            value,
            places: None,
        })
    }
    pub(super) fn value(self) -> f64 {
        self.value
    }
    pub(super) fn text(self) -> String {
        match self.places {
            Some(places) => format!("{:.places$}", self.value),
            None => self.value.to_string(),
        }
    }
}

pub fn decimal(value: f64, places: usize) -> Option<Number> {
    if places > 16 {
        return None;
    }
    Number::new(value).map(|mut number| {
        number.places = Some(places);
        number
    })
}

pub struct Numeric(pub(super) Option<Number>);
impl From<Option<Number>> for Numeric {
    fn from(value: Option<Number>) -> Self {
        Self(value)
    }
}
impl From<Number> for Numeric {
    fn from(value: Number) -> Self {
        Self(Some(value))
    }
}
macro_rules! numbers {
    ($($ty:ty),*) => { $(impl From<$ty> for Numeric {
        fn from(value: $ty) -> Self { Self(Number::new(f64::from(value))) }
    })* };
}
numbers!(f64, f32, u32, i32, u16, i16, u8, i8);

#[derive(Clone, Copy)]
pub struct UnitInterval(Number);

#[derive(Clone, Copy)]
pub enum ColorValue {
    Rgb(crate::Color),
    Gray(UnitInterval),
}
impl ColorValue {
    pub const BLACK: Self = Self::Rgb(crate::Color { r: 0, g: 0, b: 0 });
    pub const WHITE: Self = Self::Rgb(crate::Color {
        r: 255,
        g: 255,
        b: 255,
    });
    pub fn from_hex(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let rgb = u32::from_str_radix(hex, 16).ok()?;
        Some(Self::Rgb(crate::Color {
            r: (rgb >> 16) as u8,
            g: (rgb >> 8) as u8,
            b: rgb as u8,
        }))
    }
    pub fn gray(level: f64) -> Option<Self> {
        Number::new(level)
            .filter(|number| (0.0..=1.0).contains(&number.value()))
            .map(|number| Self::Gray(UnitInterval(number)))
    }
    pub(super) fn text(self) -> String {
        match self {
            Self::Rgb(color) => color_hex(&color),
            Self::Gray(level) => {
                let percent = level.0.value() * 100.;
                format!("rgb({percent:.9}%,{percent:.9}%,{percent:.9}%)")
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum Paint {
    None,
    Solid(ColorValue),
    Gradient(Definition<Gradient>),
}
impl Paint {
    pub const BLACK: Self = Self::Solid(ColorValue::BLACK);
    pub fn from_hex(text: &str) -> Option<Self> {
        if text == "none" {
            Some(Self::None)
        } else {
            ColorValue::from_hex(text).map(Self::Solid)
        }
    }
    pub(super) fn text(self) -> String {
        match self {
            Self::None => "none".into(),
            Self::Solid(color) => color.text(),
            Self::Gradient(id) => id.url(),
        }
    }
}

pub fn color_hex(color: &crate::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

macro_rules! keywords {
    ($($name:ident { $($variant:ident => $text:literal),+ $(,)? })*) => { $(
        #[derive(Clone, Copy)]
        pub enum $name { $($variant),+ }
        impl $name { pub(super) fn text(self) -> &'static str { match self { $(Self::$variant => $text),+ } } }
    )* };
}
keywords! {
    LineCap { Butt => "butt", Round => "round", Square => "square" }
    LineJoin { Miter => "miter", Round => "round", Bevel => "bevel" }
    TextAnchor { Start => "start", Middle => "middle", End => "end" }
    FontFamily { Arial => "Arial, sans-serif", Roboto => "Roboto, Arial, sans-serif" }
    TextDecoration { Underline => "underline", StrikeThrough => "line-through", Both => "underline line-through" }
    PageTemplate { Lines => "lines", Dots => "dots" }
    ObjectKind { Image => "image", Table => "table", CodeBlock => "code-block" }
    Blend { Darken => "mix-blend-mode:darken", Lighten => "mix-blend-mode:lighten" }
}

#[derive(Clone, Copy)]
pub struct StrokeIndex(pub usize);
#[derive(Clone, Copy)]
pub struct ReplayPart(pub usize);

pub struct ViewBox {
    values: [Option<Number>; 4],
}
impl ViewBox {
    pub fn new(x: f64, y: f64, width: f64, height: f64, places: usize) -> Self {
        Self {
            values: [x, y, width, height].map(|value| decimal(value, places)),
        }
    }
    pub(super) fn text(self) -> Option<String> {
        let [Some(x), Some(y), Some(width), Some(height)] = self.values else {
            return None;
        };
        if width.value() <= 0. || height.value() <= 0. {
            return None;
        }
        Some(svg::node::Value::from((x.text(), y.text(), width.text(), height.text())).to_string())
    }
}

pub struct Transform {
    kind: TransformKind,
    values: Vec<Option<Number>>,
}
enum TransformKind {
    Rotate,
    Translate,
    Scale,
    Matrix,
}
impl Transform {
    pub fn rotate(angle: f64, cx: f64, cy: f64, places: usize) -> Self {
        Self {
            kind: TransformKind::Rotate,
            values: [angle, cx, cy].map(|v| decimal(v, places)).into(),
        }
    }
    pub fn rotate_origin(angle: f64) -> Self {
        Self {
            kind: TransformKind::Rotate,
            values: vec![Number::new(angle)],
        }
    }
    pub fn translate(x: f64, y: f64, places: usize) -> Self {
        Self {
            kind: TransformKind::Translate,
            values: vec![Number::new(x), decimal(y, places)],
        }
    }
    pub fn scale(x: f64, y: f64, places: usize) -> Self {
        Self {
            kind: TransformKind::Scale,
            values: vec![Number::new(x), decimal(y, places)],
        }
    }
    pub fn matrix(values: [f64; 6], linear_places: usize, translation_places: usize) -> Self {
        Self {
            kind: TransformKind::Matrix,
            values: values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    decimal(
                        value,
                        if index < 4 {
                            linear_places
                        } else {
                            translation_places
                        },
                    )
                })
                .collect(),
        }
    }
    pub(super) fn text(self) -> Option<String> {
        let values = self
            .values
            .into_iter()
            .map(|value| value.map(Number::text))
            .collect::<Option<Vec<_>>>()?;
        let (kind, separator) = match self.kind {
            TransformKind::Rotate => ("rotate", " "),
            TransformKind::Translate => ("translate", " "),
            TransformKind::Scale => ("scale", " "),
            TransformKind::Matrix => ("matrix", ","),
        };
        Some(format!("{kind}({})", values.join(separator)))
    }
}
