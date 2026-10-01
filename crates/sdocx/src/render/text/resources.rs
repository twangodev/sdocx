use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use crate::fonts::{FontBook, FontError, ResolvedFace, UnicodeBuffer};

use super::objects::{ObjectDiagnostic, ObjectDiagnosticKind, ObjectPageOwnership};
use super::{
    PageExclusions, TextContext, TextSettings, TextStyle, VerticalExclusion, WrappedLine,
    explicit_line_height, finite_native_geometry,
};
use crate::render::vector::{EmbeddedFont, Scene};
use crate::{LineSpacingType, ParagraphLineSpacing};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextDiagnosticKind {
    UnavailableFamily,
    UnusableFontData,
    MissingGlyphs,
    MeasurementFailure,
    InvalidGeometry,
    /// The saved text frame is retained because its native template geometry is unsupported.
    UnsupportedTextFrame,
    /// Measured glyph positions cannot be reproduced by independently positioned SVG text.
    UnsupportedGlyphPositioning,
    /// Retained text measurements do not provide the highlighted range's boundaries.
    UnsupportedBackgroundPositioning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextDiagnostic {
    pub kind: TextDiagnosticKind,
    pub family: String,
    pub codepoints: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::render) enum SourceOwner {
    Text(Range<usize>),
    Object(Range<usize>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::render) struct SourceTextDiagnostic {
    pub owner: Option<SourceOwner>,
    pub diagnostic: TextDiagnostic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::render) struct SourceObjectDiagnostic {
    pub owner: Option<SourceOwner>,
    pub diagnostic: ObjectDiagnostic,
}

#[derive(Clone)]
pub(in crate::render) struct TextRenderer<'a> {
    pub settings: TextSettings,
    pub point_marker_target: crate::render::PointMarkerTarget,
    pub object_page_ownership: ObjectPageOwnership,
    pub fonts: &'a FontBook,
    default_family: &'static str,
    faces: Rc<RefCell<Vec<ResolvedFace>>>,
    diagnostics: Rc<RefCell<Vec<SourceTextDiagnostic>>>,
    object_diagnostics: Rc<RefCell<Vec<SourceObjectDiagnostic>>>,
    page_exclusions: Option<Rc<PageExclusions>>,
    source_owner: Option<SourceOwner>,
    source_owner_locked: bool,
}

impl<'a> TextRenderer<'a> {
    pub fn new(settings: TextSettings, fonts: &'a FontBook) -> Self {
        Self {
            settings,
            point_marker_target: Default::default(),
            object_page_ownership: Default::default(),
            fonts,
            default_family: "Roboto",
            faces: Default::default(),
            diagnostics: Default::default(),
            object_diagnostics: Default::default(),
            page_exclusions: None,
            source_owner: None,
            source_owner_locked: false,
        }
    }

    pub fn with_page_exclusions(mut self, exclusions: Option<PageExclusions>) -> Self {
        self.page_exclusions = exclusions.map(Rc::new);
        self
    }

    pub fn with_point_marker_target(mut self, target: crate::render::PointMarkerTarget) -> Self {
        self.point_marker_target = target;
        self
    }

    pub fn with_object_page_ownership(mut self, ownership: ObjectPageOwnership) -> Self {
        self.object_page_ownership = ownership;
        self
    }

    pub fn default_cap_height_ratio(&self) -> Option<f64> {
        self.fonts
            .resolve("sans-serif", false, false)
            .ok()?
            .metrics
            .cap_height_ratio()
    }

    pub fn for_resolved_text(&self, default_family: &'static str) -> Self {
        Self {
            settings: TextSettings::resolved(),
            point_marker_target: self.point_marker_target,
            object_page_ownership: self.object_page_ownership,
            fonts: self.fonts,
            default_family,
            faces: Rc::clone(&self.faces),
            diagnostics: Rc::clone(&self.diagnostics),
            object_diagnostics: Rc::clone(&self.object_diagnostics),
            page_exclusions: None,
            source_owner: self.source_owner.clone(),
            source_owner_locked: self.source_owner.is_some(),
        }
    }

    pub fn for_source(&self, source: Range<usize>) -> Self {
        let mut renderer = self.clone();
        if !renderer.source_owner_locked {
            renderer.source_owner = Some(SourceOwner::Text(source));
        }
        renderer
    }

    pub fn for_object_source(&self, source: Range<usize>) -> Self {
        let mut renderer = self.clone();
        if !renderer.source_owner_locked {
            renderer.source_owner = Some(SourceOwner::Object(source));
            renderer.source_owner_locked = true;
        }
        renderer
    }

    pub fn planning_scope(&self) -> Self {
        let mut renderer = self.clone();
        renderer.faces = Default::default();
        renderer.diagnostics = Default::default();
        renderer.object_diagnostics = Default::default();
        renderer
    }

    fn register_face(&self, face: &ResolvedFace) {
        let mut faces = self.faces.borrow_mut();
        if !faces.iter().any(|registered| registered.id == face.id) {
            faces.push(face.clone());
        }
    }

    pub fn object_exclusions(
        &self,
        constraint: crate::ObjectSpanLayoutConstraint,
        stored_top: f64,
        offset_y: f64,
    ) -> Vec<VerticalExclusion> {
        self.page_exclusions
            .as_ref()
            .map_or_else(Vec::new, |pages| {
                pages
                    .for_object(constraint, stored_top)
                    .into_iter()
                    .map(|band| {
                        VerticalExclusion::obstacle(band.top + offset_y, band.bottom + offset_y)
                    })
                    .collect()
            })
    }

    pub fn table_split_rects(
        &self,
        constraint: crate::ObjectSpanLayoutConstraint,
        candidate_top: f64,
    ) -> Vec<crate::BoundingBox> {
        self.page_exclusions
            .as_ref()
            .map_or_else(Vec::new, |pages| {
                pages.table_split_rects(constraint, candidate_top)
            })
    }

    pub fn resolve(&self, style: &TextStyle, context: TextContext) -> Option<ResolvedFace> {
        let family = style.family.as_deref().unwrap_or(self.default_family);
        let bold = style.bold && matches!(context, TextContext::Placed);
        match self.fonts.resolve_with_fallback(family, bold, style.italic) {
            Ok(selection) => {
                if selection.used_fallback {
                    self.record(TextDiagnostic {
                        kind: TextDiagnosticKind::UnavailableFamily,
                        family: family.into(),
                        codepoints: Vec::new(),
                    });
                }
                Some(selection.face)
            }
            Err(error) => {
                self.record(TextDiagnostic {
                    kind: match error {
                        FontError::MissingFont { .. } => TextDiagnosticKind::UnavailableFamily,
                        _ => TextDiagnosticKind::UnusableFontData,
                    },
                    family: family.into(),
                    codepoints: Vec::new(),
                });
                None
            }
        }
    }

    pub fn output_style(&self, text: &str, style: &TextStyle, context: TextContext) -> TextStyle {
        let mut output = style.clone();
        if let Some(face) = self.resolve(style, context) {
            output = self.output_style_with_face(style, &face);
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(text);
            if let Ok(run) = face.shape(buffer, &[]) {
                let mut clusters = run
                    .glyphs
                    .iter()
                    .map(|glyph| glyph.cluster as usize)
                    .collect::<Vec<_>>();
                clusters.extend([text.len()]);
                clusters.sort_unstable();
                clusters.dedup();
                let mut codepoints = Vec::new();
                for glyph in run.glyphs.iter().filter(|glyph| glyph.id == 0) {
                    let start = glyph.cluster as usize;
                    let end = clusters
                        .iter()
                        .copied()
                        .find(|index| *index > start)
                        .unwrap_or(text.len());
                    if let Some(cluster) = text.get(start..end) {
                        codepoints.extend(cluster.chars().map(u32::from));
                    }
                }
                if !codepoints.is_empty() {
                    self.record(TextDiagnostic {
                        kind: TextDiagnosticKind::MissingGlyphs,
                        family: face.family,
                        codepoints,
                    });
                }
            } else {
                self.record(TextDiagnostic {
                    kind: TextDiagnosticKind::UnusableFontData,
                    family: face.family,
                    codepoints: Vec::new(),
                });
            }
        }
        output
    }

    pub fn output_style_with_face(&self, style: &TextStyle, face: &ResolvedFace) -> TextStyle {
        self.register_face(face);
        let mut output = style.clone();
        if style.family.is_some() || face.family != "Roboto" {
            output.family = Some(face.family.clone());
        }
        output
    }

    pub fn report_resolution(&self, style: &TextStyle, context: TextContext) {
        let _ = self.resolve(style, context);
    }

    pub fn embed_fonts(&self, svg: &mut Scene) {
        for face in self.faces.borrow().iter() {
            svg.push(EmbeddedFont::new(
                &face.family,
                face.weight.0,
                face.style,
                face.bytes(),
            ));
        }
    }

    pub fn diagnostics(&self) -> Vec<TextDiagnostic> {
        let mut diagnostics = Vec::new();
        for issue in self.diagnostics.borrow().iter() {
            merge_diagnostic(&mut diagnostics, issue.diagnostic.clone());
        }
        diagnostics
    }

    pub fn scoped_diagnostics(&self) -> Vec<SourceTextDiagnostic> {
        self.diagnostics.borrow().clone()
    }

    pub fn report_text_issues(&self, issues: &[SourceTextDiagnostic]) {
        for issue in issues {
            self.record_owned(issue.clone());
        }
    }

    pub fn object_diagnostics(&self) -> Vec<ObjectDiagnostic> {
        let mut diagnostics = Vec::new();
        for issue in self.object_diagnostics.borrow().iter() {
            if !diagnostics.contains(&issue.diagnostic) {
                diagnostics.push(issue.diagnostic);
            }
        }
        diagnostics
    }

    pub fn scoped_object_diagnostics(&self) -> Vec<SourceObjectDiagnostic> {
        self.object_diagnostics.borrow().clone()
    }

    pub fn report_object_issues(&self, issues: &[ObjectDiagnostic]) {
        for issue in issues {
            self.record_object_issue(SourceObjectDiagnostic {
                owner: self.source_owner.clone(),
                diagnostic: *issue,
            });
        }
    }

    pub fn report_owned_object_issues(&self, issues: &[SourceObjectDiagnostic]) {
        for issue in issues {
            self.record_object_issue(issue.clone());
        }
    }

    fn record_object_issue(&self, issue: SourceObjectDiagnostic) {
        let mut diagnostics = self.object_diagnostics.borrow_mut();
        if !diagnostics.contains(&issue) {
            diagnostics.push(issue);
        }
    }

    pub fn report_geometry_issues(&self, issues: &[TextDiagnostic]) {
        for issue in issues {
            self.record(issue.clone());
        }
    }

    pub fn invalid_geometry(&self, family: &str) {
        self.record(TextDiagnostic {
            kind: TextDiagnosticKind::InvalidGeometry,
            family: family.into(),
            codepoints: Vec::new(),
        });
    }

    pub fn report_line_geometry(&self, line: &WrappedLine, spacing: Option<ParagraphLineSpacing>) {
        if line.has_block_margins() {
            return;
        }
        let explicit_height = spacing
            .and_then(|spacing| explicit_line_height(line.font_size, spacing, self.settings));
        let invalid_spacing = spacing.is_some_and(|spacing| {
            matches!(
                spacing.kind,
                LineSpacingType::Pixels | LineSpacingType::Percent
            ) && (spacing.value > 0.0 || !spacing.value.is_finite())
                && explicit_height.is_none()
        });
        let invalid_default_height =
            explicit_height.is_none() && finite_native_geometry(line.font_size * 1.35).is_none();
        if invalid_spacing || invalid_default_height {
            let family = line
                .placements
                .first()
                .and_then(|placement| placement.cluster.run.style.family.as_deref())
                .unwrap_or("Roboto");
            self.invalid_geometry(family);
        }
    }

    pub fn object_layout_unsupported(&self, anchor_utf16: i32) {
        self.report_object_issues(&[ObjectDiagnostic {
            anchor_utf16,
            kind: ObjectDiagnosticKind::MixedParagraphLayout,
        }]);
    }

    pub fn measurement_failed(&self, family: &str) {
        self.record(TextDiagnostic {
            kind: TextDiagnosticKind::MeasurementFailure,
            family: family.into(),
            codepoints: Vec::new(),
        });
    }

    pub fn glyph_positioning_unsupported(&self, family: &str, text: &str) {
        self.record(TextDiagnostic {
            kind: TextDiagnosticKind::UnsupportedGlyphPositioning,
            family: family.into(),
            codepoints: text.chars().map(u32::from).collect(),
        });
    }

    pub fn missing_glyphs(&self, family: &str, text: &str) {
        self.record(TextDiagnostic {
            kind: TextDiagnosticKind::MissingGlyphs,
            family: family.into(),
            codepoints: text.chars().map(u32::from).collect(),
        });
    }

    fn record(&self, diagnostic: TextDiagnostic) {
        self.record_owned(SourceTextDiagnostic {
            owner: self.source_owner.clone(),
            diagnostic,
        });
    }

    fn record_owned(&self, mut issue: SourceTextDiagnostic) {
        let mut diagnostics = self.diagnostics.borrow_mut();
        if let Some(existing) = diagnostics.iter_mut().find(|existing| {
            existing.owner == issue.owner
                && existing.diagnostic.kind == issue.diagnostic.kind
                && existing.diagnostic.family == issue.diagnostic.family
        }) {
            existing
                .diagnostic
                .codepoints
                .append(&mut issue.diagnostic.codepoints);
            existing.diagnostic.codepoints.sort_unstable();
            existing.diagnostic.codepoints.dedup();
        } else {
            issue.diagnostic.codepoints.sort_unstable();
            issue.diagnostic.codepoints.dedup();
            diagnostics.push(issue);
        }
    }
}

fn merge_diagnostic(diagnostics: &mut Vec<TextDiagnostic>, mut diagnostic: TextDiagnostic) {
    if let Some(existing) = diagnostics
        .iter_mut()
        .find(|existing| existing.kind == diagnostic.kind && existing.family == diagnostic.family)
    {
        existing.codepoints.append(&mut diagnostic.codepoints);
        existing.codepoints.sort_unstable();
        existing.codepoints.dedup();
    } else {
        diagnostics.push(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::Color;
    use crate::fonts::fontdb::Database;

    fn renderer(fonts: &FontBook) -> TextRenderer<'_> {
        TextRenderer::new(
            TextSettings {
                scale: 1.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            fonts,
        )
    }

    fn style(family: &str) -> TextStyle {
        TextStyle {
            font_size: 19.0,
            family: Some(family.into()),
            color: "#262626".into(),
            background: None,
            source_color: Color {
                r: 38,
                g: 38,
                b: 38,
            },
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            link_target: None,
        }
    }

    #[test]
    fn planning_faces_are_not_embedded_until_retained_text_is_painted() {
        let fonts = FontBook::default();
        let planner = renderer(&fonts);
        let sans = planner
            .resolve(&style("Roboto"), TextContext::Placed)
            .unwrap();
        planner
            .resolve(&style("Roboto Mono"), TextContext::Placed)
            .unwrap();
        assert!(planner.faces.borrow().is_empty());

        let page = renderer(&fonts);
        let requested = style("Missing family");
        page.report_resolution(&requested, TextContext::Placed);
        assert!(page.faces.borrow().is_empty());
        assert_eq!(
            page.diagnostics()[0].kind,
            TextDiagnosticKind::UnavailableFamily
        );
        page.output_style_with_face(&requested, &sans);
        assert_eq!(page.faces.borrow().len(), 1);
        assert_eq!(page.faces.borrow()[0].id, sans.id);
        assert!(planner.faces.borrow().is_empty());

        let generic = renderer(&fonts);
        generic.report_resolution(&style("sans-serif"), TextContext::Placed);
        assert!(generic.diagnostics().is_empty());
        assert!(generic.faces.borrow().is_empty());
    }

    #[test]
    fn source_owned_issues_are_filtered_before_page_deduplication() {
        let fonts = FontBook::default();
        let planner = renderer(&fonts);
        planner.for_source(0..4).missing_glyphs("Roboto", "中");
        planner.for_source(8..12).missing_glyphs("Roboto", "😀");
        assert_eq!(planner.diagnostics()[0].codepoints, [0x4e2d, 0x1f600]);
        let owned = planner.scoped_diagnostics();
        assert_eq!(owned.len(), 2);
        assert_eq!(owned[0].owner, Some(SourceOwner::Text(0..4)));
        assert_eq!(owned[1].owner, Some(SourceOwner::Text(8..12)));

        let second_page = renderer(&fonts);
        second_page.report_text_issues(&owned[1..]);
        second_page.report_text_issues(&owned[1..]);
        assert_eq!(second_page.diagnostics()[0].codepoints, [0x1f600]);
        assert_eq!(second_page.scoped_diagnostics(), owned[1..]);
        assert_eq!(planner.scoped_diagnostics(), owned);
    }

    #[test]
    fn child_measurement_retains_outer_object_owner_and_unknown_issues_stay_explicit() {
        let fonts = FontBook::default();
        let planner = renderer(&fonts);
        let child = planner
            .for_object_source(20..21)
            .for_resolved_text("sans-serif")
            .for_source(0..10)
            .for_object_source(3..4);
        child.measurement_failed("sans-serif");
        planner.measurement_failed("sans-serif");
        let owned = planner.scoped_diagnostics();
        assert_eq!(owned.len(), 2);
        assert_eq!(owned[0].owner, Some(SourceOwner::Object(20..21)));
        assert_eq!(owned[1].owner, None);
        assert_eq!(planner.diagnostics().len(), 1);
    }

    #[test]
    fn numeric_child_local_runs_keep_the_parent_marker_anchor() {
        let fonts = FontBook::default();
        let planner = renderer(&fonts);
        planner
            .for_source(50..51)
            .for_resolved_text("sans-serif")
            .for_source(0..2)
            .measurement_failed("sans-serif");
        assert_eq!(
            planner.scoped_diagnostics()[0].owner,
            Some(SourceOwner::Text(50..51))
        );
    }

    #[test]
    fn planning_scope_keeps_context_and_isolates_all_output_registries() {
        let fonts = FontBook::default();
        let page = renderer(&fonts)
            .with_point_marker_target(crate::render::PointMarkerTarget::Uwp)
            .for_object_source(20..21)
            .for_resolved_text("sans-serif");
        let face = fonts.resolve("Roboto", false, false).unwrap();
        page.output_style_with_face(&style("Roboto"), &face);
        page.invalid_geometry("Roboto");
        page.object_layout_unsupported(20);
        let page_text_issues = page.scoped_diagnostics();
        let page_object_issues = page.scoped_object_diagnostics();

        let planner = page.planning_scope();
        assert_eq!(planner.default_family, page.default_family);
        assert_eq!(planner.settings.scale, page.settings.scale);
        assert_eq!(
            planner.settings.font_size_delta,
            page.settings.font_size_delta
        );
        assert_eq!(planner.point_marker_target, page.point_marker_target);
        assert_eq!(planner.source_owner, page.source_owner);
        assert_eq!(planner.source_owner_locked, page.source_owner_locked);
        assert!(std::ptr::eq(planner.fonts, page.fonts));
        assert!(planner.faces.borrow().is_empty());
        assert!(planner.scoped_diagnostics().is_empty());
        assert!(planner.scoped_object_diagnostics().is_empty());

        let mono = fonts.resolve("Roboto Mono", false, false).unwrap();
        planner.output_style_with_face(&style("Roboto Mono"), &mono);
        planner.for_source(0..2).measurement_failed("sans-serif");
        planner.object_layout_unsupported(0);
        assert_eq!(planner.faces.borrow().len(), 1);
        assert_eq!(planner.faces.borrow()[0].id, mono.id);
        assert_eq!(
            planner.scoped_diagnostics()[0].owner,
            Some(SourceOwner::Object(20..21))
        );
        assert_eq!(page.faces.borrow().len(), 1);
        assert_eq!(page.faces.borrow()[0].id, face.id);
        assert_eq!(page.scoped_diagnostics(), page_text_issues);
        assert_eq!(page.scoped_object_diagnostics(), page_object_issues);
    }

    #[test]
    fn repeated_child_anchors_keep_distinct_outer_object_owners() {
        let fonts = FontBook::default();
        let planner = renderer(&fonts);
        let issue = ObjectDiagnostic {
            anchor_utf16: 0,
            kind: ObjectDiagnosticKind::MixedParagraphLayout,
        };
        planner
            .for_object_source(5..6)
            .report_object_issues(&[issue]);
        planner
            .for_object_source(20..21)
            .for_source(0..10)
            .report_object_issues(&[issue]);
        let owned = planner.scoped_object_diagnostics();
        assert_eq!(owned.len(), 2);
        assert_eq!(planner.object_diagnostics(), [issue]);
        assert_eq!(owned[0].owner, Some(SourceOwner::Object(5..6)));
        assert_eq!(owned[1].owner, Some(SourceOwner::Object(20..21)));

        let second_page = renderer(&fonts);
        second_page.report_owned_object_issues(&owned[1..]);
        assert_eq!(second_page.scoped_object_diagnostics(), owned[1..]);
        assert_eq!(second_page.object_diagnostics(), [issue]);
    }

    #[test]
    fn supplied_face_changes_family_without_resolving_or_losing_emphasis() {
        let fonts = FontBook::new(Arc::new(Database::new()));
        let renderer = renderer(&fonts);
        let face = FontBook::default()
            .resolve("Roboto Mono", false, false)
            .unwrap();
        let style = TextStyle {
            font_size: 19.0,
            family: Some("Unavailable".into()),
            color: "#123456".into(),
            background: None,
            source_color: Color {
                r: 18,
                g: 52,
                b: 86,
            },
            bold: true,
            italic: true,
            underline: true,
            strikethrough: true,
            link_target: Some("https://example.com".into()),
        };
        let output = renderer.output_style_with_face(&style, &face);
        assert_eq!(output.family.as_deref(), Some("Roboto Mono"));
        assert_eq!(output.font_size, style.font_size);
        assert_eq!(output.color, style.color);
        assert_eq!(output.source_color, style.source_color);
        assert!(output.bold && output.italic && output.underline && output.strikethrough);
        assert_eq!(output.link_target, style.link_target);
        assert!(renderer.diagnostics().is_empty());
        assert_eq!(renderer.faces.borrow().len(), 1);
        assert_eq!(renderer.faces.borrow()[0].id, face.id);
    }

    #[test]
    fn geometry_diagnostics_merge_cached_and_used_spacing_issues() {
        let fonts = FontBook::default();
        let renderer = renderer(&fonts);
        let issue = TextDiagnostic {
            kind: TextDiagnosticKind::InvalidGeometry,
            family: "Roboto".into(),
            codepoints: Vec::new(),
        };
        renderer.report_geometry_issues(std::slice::from_ref(&issue));
        renderer.report_geometry_issues(std::slice::from_ref(&issue));
        renderer.invalid_geometry("Roboto");
        assert_eq!(renderer.diagnostics(), [issue]);
    }

    #[test]
    fn margin_blocks_skip_unused_line_spacing_validation() {
        let fonts = FontBook::default();
        let renderer = renderer(&fonts);
        let spacing = Some(ParagraphLineSpacing {
            kind: LineSpacingType::Percent,
            value: f32::MAX,
        });
        let mut line = WrappedLine::unmeasured(0..1, 45.0);
        line.objects.push(super::super::wrapping::PositionedObject {
            x: 0.0,
            visual_rank: 0,
            prepared: None,
            object: super::super::objects::MeasuredObject {
                context: super::super::objects::ObjectMeasurementContext::Frame,
                source: 0..1,
                span_index: 0,
                bounds: crate::BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 100.0,
                    y_max: 80.0,
                },
                width: 100.0,
                advance: 100.0,
                height: 80.0,
                inline: false,
                left_margin: 0.0,
                top_margin: 10.0,
                bottom_margin: 10.0,
                minimum_first_page_height: None,
            },
        });
        renderer.report_line_geometry(&line, spacing);
        assert!(renderer.diagnostics().is_empty());
        line.objects[0].object.inline = true;
        renderer.report_line_geometry(&line, spacing);
        assert_eq!(
            renderer.diagnostics()[0].kind,
            TextDiagnosticKind::InvalidGeometry
        );
    }

    #[test]
    fn positioning_and_coverage_diagnostics_merge_by_family_and_reason() {
        let fonts = FontBook::default();
        let renderer = renderer(&fonts);
        renderer.glyph_positioning_unsupported("Roboto", "e\u{301}😀e");
        renderer.glyph_positioning_unsupported("Roboto", "😀中");
        renderer.glyph_positioning_unsupported("Roboto Mono", "e");
        renderer.missing_glyphs("Roboto", "中😀中");
        assert_eq!(
            renderer.diagnostics(),
            [
                TextDiagnostic {
                    kind: TextDiagnosticKind::UnsupportedGlyphPositioning,
                    family: "Roboto".into(),
                    codepoints: vec![0x65, 0x301, 0x4e2d, 0x1f600],
                },
                TextDiagnostic {
                    kind: TextDiagnosticKind::UnsupportedGlyphPositioning,
                    family: "Roboto Mono".into(),
                    codepoints: vec![0x65],
                },
                TextDiagnostic {
                    kind: TextDiagnosticKind::MissingGlyphs,
                    family: "Roboto".into(),
                    codepoints: vec![0x4e2d, 0x1f600],
                },
            ]
        );
    }

    #[test]
    fn object_diagnostics_preserve_distinct_anchors_and_reasons() {
        let fonts = FontBook::default();
        let renderer = renderer(&fonts);
        let issues = [
            ObjectDiagnostic {
                anchor_utf16: -1,
                kind: ObjectDiagnosticKind::InvalidAnchor,
            },
            ObjectDiagnostic {
                anchor_utf16: 3,
                kind: ObjectDiagnosticKind::NonReplacementAnchor,
            },
            ObjectDiagnostic {
                anchor_utf16: 3,
                kind: ObjectDiagnosticKind::UnsupportedContent,
            },
        ];
        renderer.report_object_issues(&issues);
        renderer.report_object_issues(&issues);
        renderer.object_layout_unsupported(3);
        renderer.object_layout_unsupported(3);
        renderer.object_layout_unsupported(4);
        let expected = [
            issues.to_vec(),
            vec![
                ObjectDiagnostic {
                    anchor_utf16: 3,
                    kind: ObjectDiagnosticKind::MixedParagraphLayout,
                },
                ObjectDiagnostic {
                    anchor_utf16: 4,
                    kind: ObjectDiagnosticKind::MixedParagraphLayout,
                },
            ],
        ]
        .concat();
        assert_eq!(renderer.object_diagnostics(), expected);
        assert!(renderer.diagnostics().is_empty());
    }
}
