use std::ops::{Range, RangeInclusive};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeWrapKind {
    Ordinary,
    Space,
    Tab,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
/// Ordinary entry metrics with zero native fields 72/76 and no object margins.
pub(super) struct NativeWrapMetrics {
    pub font_size: f32,
    pub height: f32,
}

impl NativeWrapMetrics {
    fn merge(self, other: Self) -> Self {
        Self {
            font_size: if other.font_size > self.font_size {
                other.font_size
            } else {
                self.font_size
            },
            height: if other.height > self.height {
                other.height
            } else {
                self.height
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeWrapEntry {
    pub advance: f32,
    pub kind: NativeWrapKind,
    pub break_end_utf16: Option<usize>,
    pub metrics: NativeWrapMetrics,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeWrapWidths {
    pub available: f32,
    pub full: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeBlock {
    pub range_utf16_inclusive: RangeInclusive<usize>,
    pub width: f32,
    pub metrics: NativeWrapMetrics,
    pub space_weight: u32,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum NativeHorizontalAlignment {
    Start,
    Right,
    Center,
}

impl NativeHorizontalAlignment {
    fn offset(self, available: f32, measured: f32) -> Result<f32, NativeWrapError> {
        let remaining = finite(available - measured)?;
        if remaining <= 0.0 {
            return Ok(0.0);
        }
        Ok(match self {
            Self::Start => 0.0,
            Self::Right => remaining,
            Self::Center => remaining * 0.5,
        })
    }
}

impl NativeBlock {
    pub fn aligned_origin(
        &self,
        left: f32,
        available: f32,
        alignment: NativeHorizontalAlignment,
    ) -> Result<f32, NativeWrapError> {
        if ![left, available, self.width]
            .into_iter()
            .all(f32::is_finite)
            || available < 0.0
        {
            return Err(NativeWrapError::InvalidGeometry);
        }
        let block_left = alignment.offset(available, self.width)?;
        let block_right = finite(self.width + block_left)?;
        let translated_width = finite(block_right - block_left)?;
        finite(left + alignment.offset(available, translated_width)?)
    }

    pub fn justification_share(&self, available: f32) -> Result<f32, NativeWrapError> {
        if !available.is_finite() || self.space_weight == 0 || self.space_weight > i32::MAX as u32 {
            return Err(NativeWrapError::InvalidGeometry);
        }
        finite((available - self.width) / self.space_weight as f32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum NativeWrapError {
    #[error("native wrapping requires a nonempty bounded UTF16 range")]
    InvalidRange,
    #[error("native wrapping requires finite nonnegative widths and metrics")]
    InvalidGeometry,
    #[error("native wrapping requires finite UTF16 advances")]
    InvalidAdvance,
    #[error("native wrapping break ends must follow their slot and lie within the source")]
    InvalidBreakEnd,
    #[error("native wrapping arithmetic exceeded its finite numeric domain")]
    NumericOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NativeWrapOperation {
    Candidate {
        entry_utf16: usize,
        committed: f32,
        pending: f32,
        advance: f32,
        base: f32,
        candidate: f32,
        available: f32,
    },
    Commit {
        entry_utf16: usize,
        reason: NativeCommitReason,
        committed_before: f32,
        pending: f32,
        committed_after: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeCommitReason {
    BreakEnd,
    Space,
    Tab,
}

pub(super) fn select_native_block(
    entries: &[NativeWrapEntry],
    requested: Range<usize>,
    widths: NativeWrapWidths,
) -> Result<Option<NativeBlock>, NativeWrapError> {
    select_with_operations(entries, requested, widths, |_| {})
}

fn select_with_operations(
    entries: &[NativeWrapEntry],
    requested: Range<usize>,
    widths: NativeWrapWidths,
    observe: impl FnMut(NativeWrapOperation),
) -> Result<Option<NativeBlock>, NativeWrapError> {
    select_with_feedback(
        entries,
        requested,
        widths,
        None,
        |_, _, advance| Ok(advance),
        observe,
    )
}

pub(super) fn select_native_block_with_objects(
    entries: &[NativeWrapEntry],
    requested: Range<usize>,
    widths: NativeWrapWidths,
    objects: &[bool],
    mut prepare: impl FnMut(usize, f32) -> Result<f32, NativeWrapError>,
) -> Result<Option<NativeBlock>, NativeWrapError> {
    select_with_feedback(
        entries,
        requested,
        widths,
        Some(objects),
        |index, base, _| prepare(index, base),
        |_| {},
    )
}

fn select_with_feedback(
    entries: &[NativeWrapEntry],
    requested: Range<usize>,
    widths: NativeWrapWidths,
    objects: Option<&[bool]>,
    mut prepare: impl FnMut(usize, f32, f32) -> Result<f32, NativeWrapError>,
    mut observe: impl FnMut(NativeWrapOperation),
) -> Result<Option<NativeBlock>, NativeWrapError> {
    if requested.is_empty() || requested.end > entries.len() || entries.len() > i32::MAX as usize {
        return Err(NativeWrapError::InvalidRange);
    }
    if objects.is_some_and(|objects| objects.len() != entries.len()) {
        return Err(NativeWrapError::InvalidRange);
    }
    if ![widths.available, widths.full]
        .into_iter()
        .all(|value| value.is_finite() && value >= 0.0)
        || widths.available > widths.full
    {
        return Err(NativeWrapError::InvalidGeometry);
    }
    for (index, &entry) in entries
        .iter()
        .enumerate()
        .take(requested.end)
        .skip(requested.start)
    {
        if !entry.advance.is_finite() {
            return Err(NativeWrapError::InvalidAdvance);
        }
        if objects.is_some_and(|objects| objects[index]) && entry.kind != NativeWrapKind::Ordinary {
            return Err(NativeWrapError::InvalidGeometry);
        }
        if ![entry.metrics.font_size, entry.metrics.height]
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0)
        {
            return Err(NativeWrapError::InvalidGeometry);
        }
        if entry
            .break_end_utf16
            .is_some_and(|end| end <= index || end > entries.len())
        {
            return Err(NativeWrapError::InvalidBreakEnd);
        }
    }
    let mut committed = 0.0_f32;
    let mut pending = 0.0_f32;
    let mut committed_metrics = NativeWrapMetrics::default();
    let mut pending_metrics = NativeWrapMetrics::default();
    let mut last_committed = None;
    let mut space_weight = 0_u32;
    for (index, entry) in entries
        .iter()
        .enumerate()
        .take(requested.end)
        .skip(requested.start)
    {
        let base = finite(committed + pending)?;
        let candidate = finite(base + entry.advance)?;
        observe(NativeWrapOperation::Candidate {
            entry_utf16: index,
            committed,
            pending,
            advance: entry.advance,
            base,
            candidate,
            available: widths.available,
        });
        let object = objects.is_some_and(|objects| objects[index]);
        let first_object = object && index == requested.start && widths.available >= widths.full;
        let advance = if object && (candidate <= widths.available || first_object) {
            finite(prepare(index, base, entry.advance)?)?
        } else {
            entry.advance
        };
        if candidate > widths.available && !first_object {
            if let Some(end) = last_committed.filter(|end| *end >= 1) {
                return Ok(Some(NativeBlock {
                    range_utf16_inclusive: requested.start..=end,
                    width: committed,
                    metrics: committed_metrics,
                    space_weight,
                }));
            }
            if !object && index == requested.start && finite(pending + entry.advance)? > widths.full
            {
                return Ok(Some(NativeBlock {
                    range_utf16_inclusive: requested.start..=index,
                    width: finite(pending + entry.advance)?,
                    metrics: entry.metrics,
                    space_weight,
                }));
            }
            return Ok((index > requested.start).then(|| NativeBlock {
                range_utf16_inclusive: requested.start..=index - 1,
                width: base,
                metrics: committed_metrics.merge(pending_metrics),
                space_weight,
            }));
        }
        pending_metrics = pending_metrics.merge(entry.metrics);
        pending = finite(pending + advance)?;
        let mut commit = |reason| -> Result<(), NativeWrapError> {
            let committed_before = committed;
            committed = finite(committed + pending)?;
            observe(NativeWrapOperation::Commit {
                entry_utf16: index,
                reason,
                committed_before,
                pending,
                committed_after: committed,
            });
            pending = 0.0;
            committed_metrics = committed_metrics.merge(pending_metrics);
            pending_metrics = NativeWrapMetrics::default();
            last_committed = Some(index);
            Ok(())
        };
        if entry.break_end_utf16 == Some(index + 1) {
            commit(NativeCommitReason::BreakEnd)?;
        }
        let (reason, weight) = match entry.kind {
            NativeWrapKind::Ordinary => continue,
            NativeWrapKind::Space => (NativeCommitReason::Space, 1),
            NativeWrapKind::Tab => (NativeCommitReason::Tab, 4),
        };
        commit(reason)?;
        space_weight = space_weight
            .checked_add(weight)
            .ok_or(NativeWrapError::NumericOverflow)?;
    }
    Ok(Some(NativeBlock {
        range_utf16_inclusive: requested.start..=requested.end - 1,
        width: finite(pending + committed)?,
        metrics: committed_metrics.merge(pending_metrics),
        space_weight,
    }))
}

fn finite(value: f32) -> Result<f32, NativeWrapError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(NativeWrapError::NumericOverflow)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeEntryPosition {
    pub left: f32,
    pub right: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeEntryCursor {
    x: f32,
}

impl NativeEntryCursor {
    pub fn new(x: f32) -> Result<Self, NativeWrapError> {
        Ok(Self { x: finite(x)? })
    }

    pub fn advance(&mut self, advance: f32) -> Result<NativeEntryPosition, NativeWrapError> {
        self.advance_justified(advance, NativeWrapKind::Ordinary, 0.0)
    }

    pub fn advance_justified(
        &mut self,
        advance: f32,
        kind: NativeWrapKind,
        share: f32,
    ) -> Result<NativeEntryPosition, NativeWrapError> {
        if !advance.is_finite() || !share.is_finite() {
            return Err(NativeWrapError::InvalidGeometry);
        }
        let left = self.x;
        let base = finite(left + advance)?;
        let right = if share == 0.0 {
            base
        } else {
            finite(match kind {
                NativeWrapKind::Ordinary => base,
                NativeWrapKind::Space => share + base,
                NativeWrapKind::Tab => share.mul_add(4.0, base),
            })?
        };
        self.x = right;
        Ok(NativeEntryPosition { left, right })
    }
}

#[cfg(test)]
#[path = "native_wrap/fixture_tests.rs"]
mod fixture_tests;

#[cfg(test)]
#[path = "native_wrap/object_fixture_tests.rs"]
mod object_fixture_tests;
