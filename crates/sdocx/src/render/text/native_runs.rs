use std::ops::RangeInclusive;

use super::{NativeDrawSpan, NativeIdentityUnavailable};

const MAX_UTF16_ENTRIES: usize = 250_000;
const MAX_CACHED_GLYPHS: usize = 1_000_000;

#[cfg(all(test, feature = "serde"))]
mod fixture_tests;

#[derive(Debug, Clone, Copy)]
pub(super) enum NativeFontState<'a> {
    Missing,
    #[cfg_attr(not(test), allow(dead_code))]
    Known {
        source_id: i32,
        bitmap: bool,
        language: &'a str,
    },
    Unavailable,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeHorizontalGeometry {
    pub x: f32,
    pub advance: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeBoundaryEntry<'a> {
    pub kind: Option<u32>,
    pub direction: Option<i32>,
    pub horizontal: Option<NativeHorizontalGeometry>,
    pub span: Result<&'a NativeDrawSpan, NativeIdentityUnavailable>,
    pub font: NativeFontState<'a>,
}

impl<'a> NativeBoundaryEntry<'a> {
    pub(super) fn from_span(span: &'a Result<NativeDrawSpan, NativeIdentityUnavailable>) -> Self {
        Self {
            kind: None,
            direction: None,
            horizontal: None,
            span: span.as_ref().map_err(|error| *error),
            font: NativeFontState::Unavailable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeRunBoundary {
    Join,
    Split,
    Unavailable,
}

pub(super) fn native_run_boundary(
    previous: NativeBoundaryEntry<'_>,
    current: NativeBoundaryEntry<'_>,
) -> NativeRunBoundary {
    if matches!(previous.font, NativeFontState::Missing)
        || matches!(current.font, NativeFontState::Missing)
        || previous.kind == Some(5)
        || current.kind == Some(5)
    {
        return NativeRunBoundary::Split;
    }
    let (Ok(left), Ok(right)) = (previous.span, current.span) else {
        return NativeRunBoundary::Split;
    };
    if left != right {
        return NativeRunBoundary::Split;
    }
    if let (Some(left), Some(right)) = (previous.direction, current.direction)
        && left != right
    {
        return NativeRunBoundary::Split;
    }
    if let (Some(left), Some(right), Some(direction)) =
        (previous.horizontal, current.horizontal, previous.direction)
    {
        let adjacent = if direction == 0 {
            left.x + left.advance == right.x
        } else {
            right.x + right.advance == left.x
        };
        if !adjacent {
            return NativeRunBoundary::Split;
        }
    }
    if let NativeFontState::Known {
        bitmap, language, ..
    } = previous.font
        && (bitmap || language == "und-Deva")
    {
        return NativeRunBoundary::Split;
    }
    let (
        NativeFontState::Known {
            source_id: left, ..
        },
        NativeFontState::Known {
            source_id: right, ..
        },
    ) = (previous.font, current.font)
    else {
        return NativeRunBoundary::Unavailable;
    };
    if left != right {
        return NativeRunBoundary::Split;
    }
    if previous.kind.is_none()
        || current.kind.is_none()
        || previous.direction.is_none()
        || current.direction.is_none()
        || previous.horizontal.is_none()
        || current.horizontal.is_none()
    {
        NativeRunBoundary::Unavailable
    } else {
        NativeRunBoundary::Join
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct NativeRect(pub [f32; 4]);

impl NativeRect {
    fn offset(self, x: f32, y: f32) -> Result<Self, NativeRunError> {
        let [left, top, right, bottom] = self.0;
        Ok(Self([
            finite(left + x)?,
            finite(top + y)?,
            finite(right + x)?,
            finite(bottom + y)?,
        ]))
    }

    fn is_empty(self) -> bool {
        self.0[0] >= self.0[2] || self.0[1] >= self.0[3]
    }

    fn union(&mut self, incoming: Self) {
        if incoming.is_empty() {
            return;
        }
        if self.is_empty() {
            *self = incoming;
        } else {
            if incoming.0[0] < self.0[0] {
                self.0[0] = incoming.0[0];
            }
            if incoming.0[1] < self.0[1] {
                self.0[1] = incoming.0[1];
            }
            if incoming.0[2] > self.0[2] {
                self.0[2] = incoming.0[2];
            }
            if incoming.0[3] > self.0[3] {
                self.0[3] = incoming.0[3];
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeCachedGlyph {
    pub payload: usize,
    pub offset: [f32; 2],
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeGlyphCache<'a> {
    pub drawable: bool,
    pub glyphs: &'a [NativeCachedGlyph],
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeRunEntry<'a> {
    pub kind: u32,
    pub direction: i32,
    pub advance: f32,
    pub position: [f32; 2],
    pub layout: NativeRect,
    pub ink: NativeRect,
    pub cache: NativeGlyphCache<'a>,
    pub span: &'a NativeDrawSpan,
    pub font: NativeFontState<'a>,
    pub paragraph_override: bool,
}

impl<'a> NativeRunEntry<'a> {
    fn boundary(&self) -> NativeBoundaryEntry<'a> {
        NativeBoundaryEntry {
            kind: Some(self.kind),
            direction: Some(self.direction),
            horizontal: Some(NativeHorizontalGeometry {
                x: self.position[0],
                advance: self.advance,
            }),
            span: Ok(self.span),
            font: self.font,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct NativeRunOffset {
    pub x: f32,
    pub y: f32,
    pub gravity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NativeEmittedGlyph {
    pub payload: usize,
    pub owner_utf16: usize,
    pub x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NativeRunPaint {
    pub font_size: f32,
    pub foreground: u32,
    pub style_bits: u8,
    pub background: u32,
    pub object: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeEmittedKind {
    Glyphs,
    DefaultEmpty,
}

#[derive(Debug, PartialEq)]
pub(super) struct NativeEmittedRun {
    pub source: RangeInclusive<usize>,
    pub glyphs: Vec<NativeEmittedGlyph>,
    pub origin: [f32; 2],
    pub layout: NativeRect,
    pub ink: NativeRect,
    pub font_id: i32,
    pub paint: NativeRunPaint,
    pub kind: NativeEmittedKind,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(super) enum NativeRunError {
    #[error("cached text source range is invalid")]
    InvalidRange,
    #[error("cached text exceeds the Rust entry or glyph budget")]
    BudgetExceeded,
    #[error("cached text contains invalid or overflowing float geometry")]
    InvalidGeometry,
    #[error("cached text entry kind {0} has no supported emission contract")]
    UnsupportedKind(u32),
    #[error("cached text owner {0} has no glyph payload")]
    MissingGlyph(usize),
    #[error("cached text owner {0} has unavailable font metadata")]
    UnavailableFont(usize),
}

fn finite(value: f32) -> Result<f32, NativeRunError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(NativeRunError::InvalidGeometry)
}

struct PendingRun<'a> {
    start: usize,
    previous: Option<usize>,
    glyphs: Vec<NativeEmittedGlyph>,
    origin: [f32; 2],
    layout: NativeRect,
    ink: NativeRect,
    span: Option<&'a NativeDrawSpan>,
    font: NativeFontState<'a>,
}

impl<'a> PendingRun<'a> {
    fn new(start: usize) -> Self {
        Self {
            start,
            previous: None,
            glyphs: Vec::new(),
            origin: [0.0; 2],
            layout: NativeRect::default(),
            ink: NativeRect::default(),
            span: None,
            font: NativeFontState::Missing,
        }
    }

    fn append(
        &mut self,
        end: usize,
        entries: &[NativeRunEntry<'_>],
    ) -> Result<NativeEmittedRun, NativeRunError> {
        if entries[self.start].direction != 0 {
            self.glyphs.reverse();
        }
        let font_id = match self.font {
            NativeFontState::Missing => -1,
            NativeFontState::Known { source_id, .. } => source_id,
            NativeFontState::Unavailable => {
                return Err(NativeRunError::UnavailableFont(self.start));
            }
        };
        let mut paint = self.span.map_or(
            NativeRunPaint {
                font_size: 17.0,
                foreground: 0xff00_0000,
                style_bits: 0,
                background: 0,
                object: false,
            },
            |span| NativeRunPaint {
                font_size: span.font_size,
                foreground: span.foreground,
                style_bits: span.style_bits,
                background: span.background,
                object: span.flags & 2 != 0,
            },
        );
        if self.span.is_some_and(|span| span.flags & 1 != 0) {
            paint.foreground = 0xff00_54ff;
            paint.style_bits |= 4;
        }
        if entries[self.start].paragraph_override {
            paint.foreground = (paint.foreground & 0x00ff_ffff) | 0x6600_0000;
            paint.style_bits |= 8;
        }
        Ok(NativeEmittedRun {
            source: self.start..=end,
            glyphs: std::mem::take(&mut self.glyphs),
            origin: self.origin,
            layout: self.layout,
            ink: self.ink,
            font_id,
            paint,
            kind: if self.span.is_none() {
                NativeEmittedKind::DefaultEmpty
            } else {
                NativeEmittedKind::Glyphs
            },
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn native_runs(
    entries: &[NativeRunEntry<'_>],
    range: RangeInclusive<usize>,
    offset: NativeRunOffset,
) -> Result<Vec<NativeEmittedRun>, NativeRunError> {
    if entries.len() > MAX_UTF16_ENTRIES {
        return Err(NativeRunError::BudgetExceeded);
    }
    let (start, end) = (*range.start(), *range.end());
    if start > end || end >= entries.len() {
        return Err(NativeRunError::InvalidRange);
    }
    let glyph_count = entries.iter().try_fold(0_usize, |total, entry| {
        total.checked_add(entry.cache.glyphs.len())
    });
    if glyph_count.is_none_or(|count| count > MAX_CACHED_GLYPHS) {
        return Err(NativeRunError::BudgetExceeded);
    }
    finite(offset.x)?;
    finite(offset.y)?;
    finite(offset.gravity)?;
    let y_offset = finite(offset.gravity + offset.y)?;
    let mut pending = PendingRun::new(start);
    let mut output = Vec::new();
    for (index, entry) in entries.iter().enumerate().take(end + 1).skip(start) {
        if !matches!(entry.kind, 0..=3 | 5) {
            return Err(NativeRunError::UnsupportedKind(entry.kind));
        }
        if !entry.cache.drawable && entry.kind != 5 {
            continue;
        }
        if entry.cache.glyphs.is_empty() {
            return Err(NativeRunError::MissingGlyph(index));
        }
        if matches!(entry.font, NativeFontState::Unavailable) {
            return Err(NativeRunError::UnavailableFont(index));
        }
        finite(entry.span.font_size)?;
        finite(entry.advance)?;
        finite(entry.position[0] + entry.advance)?;
        for value in entry
            .position
            .into_iter()
            .chain(entry.layout.0)
            .chain(entry.ink.0)
        {
            finite(value)?;
        }
        let boundary = pending
            .previous
            .map_or(NativeRunBoundary::Split, |previous| {
                native_run_boundary(entries[previous].boundary(), entry.boundary())
            });
        if index > start && boundary != NativeRunBoundary::Join {
            output.push(pending.append(index - 1, entries)?);
            pending.start = index;
        }
        let layout = entry.layout.offset(offset.x, y_offset)?;
        let ink = entry.ink.offset(offset.x, y_offset)?;
        if index == pending.start {
            pending.origin = [
                finite(entry.position[0] + offset.x)?,
                finite(entry.position[1] + y_offset)?,
            ];
            pending.layout = layout;
            pending.ink = ink;
        } else {
            pending.layout.union(layout);
            pending.ink.union(ink);
        }
        for glyph in entry.cache.glyphs {
            finite(glyph.offset[0])?;
            pending.glyphs.push(NativeEmittedGlyph {
                payload: glyph.payload,
                owner_utf16: index,
                x: finite(entry.position[0] + glyph.offset[0])?,
            });
        }
        pending.previous = Some(index);
        pending.span = Some(entry.span);
        pending.font = entry.font;
    }
    output.push(pending.append(end, entries)?);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> NativeDrawSpan {
        NativeDrawSpan {
            font_size: 17.0,
            foreground: 0xff00_0000,
            background: 0,
            composing_background: 0,
            style_bits: 0,
            family: None,
            underline: 0xff00_0000,
            correction_foreground: 0xff00_0000,
            flags: 0,
            correction_foreground_enabled: false,
        }
    }

    fn entry<'a>(span: &'a NativeDrawSpan, glyphs: &'a [NativeCachedGlyph]) -> NativeRunEntry<'a> {
        NativeRunEntry {
            kind: 0,
            direction: 0,
            advance: 10.0,
            position: [0.0, 17.0],
            layout: NativeRect([0.0, 0.0, 10.0, 20.0]),
            ink: NativeRect([0.0, 2.0, 9.0, 18.0]),
            cache: NativeGlyphCache {
                drawable: true,
                glyphs,
            },
            span,
            font: NativeFontState::Known {
                source_id: 7,
                bitmap: false,
                language: "en",
            },
            paragraph_override: false,
        }
    }

    #[test]
    fn partial_identity_never_claims_a_complete_native_join() {
        let first = Ok(span());
        assert_eq!(
            native_run_boundary(
                NativeBoundaryEntry::from_span(&first),
                NativeBoundaryEntry::from_span(&first)
            ),
            NativeRunBoundary::Unavailable
        );
        let mut different = span();
        different.composing_background = 1;
        let second = Ok(different);
        assert_eq!(
            native_run_boundary(
                NativeBoundaryEntry::from_span(&first),
                NativeBoundaryEntry::from_span(&second)
            ),
            NativeRunBoundary::Split
        );
        let unknown = Err(NativeIdentityUnavailable::UnsupportedCorrection);
        assert_eq!(
            native_run_boundary(
                NativeBoundaryEntry::from_span(&unknown),
                NativeBoundaryEntry::from_span(&unknown)
            ),
            NativeRunBoundary::Split
        );
    }

    #[test]
    fn font_wrapper_presence_and_previous_font_gates_control_boundaries() {
        let span = span();
        let mut previous = entry(&span, &[]).boundary();
        let mut current = previous;
        current.horizontal = Some(NativeHorizontalGeometry {
            x: 10.0,
            advance: 10.0,
        });
        previous.font = NativeFontState::Known {
            source_id: -1,
            bitmap: false,
            language: "",
        };
        current.font = previous.font;
        assert_eq!(
            native_run_boundary(previous, current),
            NativeRunBoundary::Join
        );
        current.font = NativeFontState::Missing;
        assert_eq!(
            native_run_boundary(previous, current),
            NativeRunBoundary::Split
        );
        current.font = NativeFontState::Known {
            source_id: -1,
            bitmap: true,
            language: "und-Deva",
        };
        assert_eq!(
            native_run_boundary(previous, current),
            NativeRunBoundary::Join
        );
        previous.font = current.font;
        assert_eq!(
            native_run_boundary(previous, current),
            NativeRunBoundary::Split
        );
        previous.font = NativeFontState::Known {
            source_id: -1,
            bitmap: false,
            language: "und-DevaX",
        };
        assert_eq!(
            native_run_boundary(previous, current),
            NativeRunBoundary::Join
        );
        previous.font = NativeFontState::Known {
            source_id: -1,
            bitmap: false,
            language: "und-Deva",
        };
        assert_eq!(
            native_run_boundary(previous, current),
            NativeRunBoundary::Split
        );
    }

    #[test]
    fn rectangle_union_preserves_signed_zero_on_equal_edges() {
        let mut rectangle = NativeRect([-0.0, -0.0, 10.0, 10.0]);
        rectangle.union(NativeRect([0.0, 0.0, 10.0, 10.0]));
        assert_eq!(rectangle.0[0].to_bits(), (-0.0_f32).to_bits());
        assert_eq!(rectangle.0[1].to_bits(), (-0.0_f32).to_bits());
        let original = rectangle;
        rectangle.union(NativeRect([20.0, 20.0, 20.0, 30.0]));
        assert_eq!(rectangle, original);
        let mut empty = NativeRect([8.0, 2.0, 8.0, 20.0]);
        empty.union(original);
        assert_eq!(empty, original);
    }

    #[test]
    fn invalid_inputs_fail_before_emitting_unsafe_native_records() {
        let span = span();
        let glyphs = [NativeCachedGlyph {
            payload: 0,
            offset: [0.0, f32::NAN],
        }];
        let original = entry(&span, &glyphs);
        let emit = |entry| native_runs(&[entry], 0..=0, NativeRunOffset::default());
        assert!(
            emit(original).is_ok(),
            "cached Y is not read by the native emitter"
        );
        assert_eq!(
            native_runs(
                &[original],
                RangeInclusive::new(1, 0),
                NativeRunOffset::default()
            ),
            Err(NativeRunError::InvalidRange)
        );
        assert_eq!(
            native_runs(&[original], 0..=usize::MAX, NativeRunOffset::default()),
            Err(NativeRunError::InvalidRange)
        );
        assert_eq!(
            emit(NativeRunEntry {
                kind: 4,
                ..original
            }),
            Err(NativeRunError::UnsupportedKind(4))
        );
        assert_eq!(
            emit(NativeRunEntry {
                font: NativeFontState::Unavailable,
                ..original
            }),
            Err(NativeRunError::UnavailableFont(0))
        );
        let empty = NativeRunEntry {
            cache: NativeGlyphCache {
                drawable: true,
                glyphs: &[],
            },
            ..original
        };
        assert_eq!(emit(empty), Err(NativeRunError::MissingGlyph(0)));
        let skipped = NativeRunEntry {
            kind: 3,
            cache: NativeGlyphCache {
                drawable: false,
                glyphs: &[],
            },
            ..original
        };
        assert_eq!(
            emit(skipped).unwrap()[0].kind,
            NativeEmittedKind::DefaultEmpty
        );
        assert_eq!(
            emit(NativeRunEntry {
                advance: f32::MAX,
                position: [f32::MAX, 17.0],
                ..original
            }),
            Err(NativeRunError::InvalidGeometry)
        );
        assert_eq!(
            native_runs(
                &[original],
                0..=0,
                NativeRunOffset {
                    y: f32::MAX,
                    gravity: f32::MAX,
                    ..Default::default()
                }
            ),
            Err(NativeRunError::InvalidGeometry)
        );
        let mut invalid = span.clone();
        invalid.font_size = f32::INFINITY;
        assert_eq!(
            emit(NativeRunEntry {
                span: &invalid,
                ..original
            }),
            Err(NativeRunError::InvalidGeometry)
        );
        assert_eq!(
            native_runs(
                &vec![original; MAX_UTF16_ENTRIES + 1],
                0..=0,
                NativeRunOffset::default()
            ),
            Err(NativeRunError::BudgetExceeded)
        );
        let repeated_glyphs = vec![glyphs[0]; 1_001];
        let large = NativeRunEntry {
            cache: NativeGlyphCache {
                drawable: true,
                glyphs: &repeated_glyphs,
            },
            ..original
        };
        assert_eq!(
            native_runs(&vec![large; 1_000], 0..=0, NativeRunOffset::default()),
            Err(NativeRunError::BudgetExceeded)
        );
    }
}
