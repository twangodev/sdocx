use crate::{BoundingBox, RichTextTable, TableBorder, TableEdgeStyle};

use super::ObjectDiagnosticKind;
use super::grid::{CellPosition, TableGrid};

const MAX_PAINT_SEGMENTS: usize = 1_048_576;
const DEFAULT_EDGE: TableEdgeStyle = TableEdgeStyle {
    color: 0xff000000,
    width: 1.0,
    start_radius: 0.0,
    end_radius: 0.0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::render) enum Edge {
    Left = 1,
    Top = 2,
    Right = 4,
    Bottom = 8,
}

impl Edge {
    const ALL: [Self; 4] = [Self::Left, Self::Top, Self::Right, Self::Bottom];

    fn style(self, border: &TableBorder) -> TableEdgeStyle {
        match self {
            Self::Left => border.left,
            Self::Top => border.top,
            Self::Right => border.right,
            Self::Bottom => border.bottom,
        }
    }

    pub fn width(self, border: Option<&TableBorder>) -> f64 {
        f64::from(
            border
                .map_or(DEFAULT_EDGE, |border| self.style(border))
                .width,
        )
    }

    fn endpoints(self, rect: [f32; 4], outer: bool) -> [f32; 4] {
        let [left, top, right, bottom] = rect;
        match (self, outer) {
            (Self::Left, true) => [left, bottom, left, top],
            (Self::Left, false) => [left, top, left, bottom],
            (Self::Top, _) => [left, top, right, top],
            (Self::Right, _) => [right, top, right, bottom],
            (Self::Bottom, true) => [right, bottom, left, bottom],
            (Self::Bottom, false) => [left, bottom, right, bottom],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::render) struct BorderPath {
    pub edge: Edge,
    pub style: TableEdgeStyle,
    pub endpoints: [f32; 4],
}

impl BorderPath {
    pub fn on_frame(self, frame: BoundingBox) -> Self {
        Self {
            endpoints: self.edge.endpoints(native_rect(frame), false),
            ..self
        }
    }

    pub fn paint_width(self) -> Option<f32> {
        (self.style.color != 0 && self.style.width != 0.0).then(|| self.style.width.max(1.0))
    }

    pub fn selected(
        self,
        position: CellPosition,
        shape: [usize; 2],
        outer: bool,
        gap: bool,
    ) -> bool {
        match self.edge {
            Edge::Left => !outer || position.column != 0,
            Edge::Top => !outer || position.row != 0,
            Edge::Right => !outer && position.column + 1 == shape[1],
            Edge::Bottom => gap || (!outer && position.row + 1 == shape[0]),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(in crate::render) struct OutlineStyle {
    pub color: u32,
    pub width: f32,
    pub rx: f32,
    pub ry: f32,
}

impl OutlineStyle {
    pub fn active(self) -> bool {
        self.color != 0 && self.width > 0.0
    }

    pub fn rounded(self) -> bool {
        self.rx > 0.0 && self.ry > 0.0
    }
}

pub(in crate::render) struct TableBorderGeometry<'a> {
    table: &'a RichTextTable,
    grid: &'a TableGrid,
    rect: [f32; 4],
    row_ends: Vec<f32>,
    column_ends: Vec<f32>,
    default_cell: Option<TableEdgeStyle>,
    default_outer: Option<TableEdgeStyle>,
}

impl<'a> TableBorderGeometry<'a> {
    pub fn new(
        table: &'a RichTextTable,
        grid: &'a TableGrid,
    ) -> Result<Self, ObjectDiagnosticKind> {
        Self::with_defaults(table, grid, Some(DEFAULT_EDGE), Some(DEFAULT_EDGE))
    }

    fn with_defaults(
        table: &'a RichTextTable,
        grid: &'a TableGrid,
        default_cell: Option<TableEdgeStyle>,
        default_outer: Option<TableEdgeStyle>,
    ) -> Result<Self, ObjectDiagnosticKind> {
        let rect = native_rect(table.style.content_bbox.unwrap_or(table.bbox));
        if !rect.into_iter().all(f32::is_finite) || rect[2] < rect[0] || rect[3] < rect[1] {
            return Err(ObjectDiagnosticKind::InvalidBounds);
        }
        let row_ends = prefix_ends(rect[1], table.rows.iter().map(|row| row.height))?;
        let column_ends = prefix_ends(rect[0], table.column_widths.iter().copied())?;
        for border in table
            .style
            .border
            .iter()
            .chain(&table.style.default_cell_border)
            .chain(
                table
                    .rows
                    .iter()
                    .flat_map(|row| &row.cells)
                    .filter_map(|cell| cell.border.as_ref()),
            )
        {
            if Edge::ALL.into_iter().any(|edge| {
                let style = edge.style(border);
                ![style.width, style.start_radius, style.end_radius]
                    .into_iter()
                    .all(f32::is_finite)
            }) {
                return Err(ObjectDiagnosticKind::InvalidBounds);
            }
        }
        let mut segments = 0_usize;
        for &position in grid.visible_cells() {
            let owner = grid
                .frame_owner(position)
                .ok_or(ObjectDiagnosticKind::UnsupportedContent)?;
            let cell = &table.rows[owner.row].cells[owner.column];
            segments = segments
                .checked_add(2 * (cell.row_span as usize + cell.column_span as usize))
                .filter(|&segments| segments <= MAX_PAINT_SEGMENTS)
                .ok_or(ObjectDiagnosticKind::UnsupportedContent)?;
        }
        Ok(Self {
            table,
            grid,
            rect,
            row_ends,
            column_ends,
            default_cell,
            default_outer,
        })
    }

    pub fn cell_paths(&self, position: CellPosition) -> Vec<BorderPath> {
        let Some(owner) = self.grid.frame_owner(position) else {
            return Vec::new();
        };
        let cell = &self.table.rows[owner.row].cells[owner.column];
        let row_end = owner.row + cell.row_span as usize;
        let column_end = owner.column + cell.column_span as usize;
        let left = self.column_ends[owner.column] - self.table.column_widths[owner.column];
        let top = self.row_ends[owner.row] - self.table.rows[owner.row].height;
        let right = self.column_ends[column_end - 1];
        let bottom = self.row_ends[row_end - 1];
        let mut paths =
            Vec::with_capacity(2 * (cell.row_span as usize + cell.column_span as usize));
        for edge in Edge::ALL {
            match edge {
                Edge::Left | Edge::Right => {
                    let column = if edge == Edge::Left {
                        owner.column
                    } else {
                        column_end - 1
                    };
                    let x = if edge == Edge::Left { left } else { right };
                    for row in owner.row..row_end {
                        let start = self.row_ends[row] - self.table.rows[row].height;
                        let end = start + self.table.rows[row].height;
                        if let Some(style) = self.cell_style(CellPosition { row, column }, edge) {
                            paths.push(BorderPath {
                                edge,
                                style,
                                endpoints: [x, start, x, end],
                            });
                        }
                    }
                }
                Edge::Top | Edge::Bottom => {
                    let row = if edge == Edge::Top {
                        owner.row
                    } else {
                        row_end - 1
                    };
                    let y = if edge == Edge::Top { top } else { bottom };
                    for column in owner.column..column_end {
                        let start = self.column_ends[column] - self.table.column_widths[column];
                        let end = start + self.table.column_widths[column];
                        if let Some(style) = self.cell_style(CellPosition { row, column }, edge) {
                            paths.push(BorderPath {
                                edge,
                                style,
                                endpoints: [start, y, end, y],
                            });
                        }
                    }
                }
            }
        }
        paths
    }

    fn cell_style(&self, position: CellPosition, edge: Edge) -> Option<TableEdgeStyle> {
        self.table.rows[position.row].cells[position.column]
            .border
            .as_ref()
            .or(self.table.style.default_cell_border.as_ref())
            .map(|border| edge.style(border))
            .or(self.default_cell)
    }

    pub fn outer_paths(&self, frame: Option<BoundingBox>) -> Vec<BorderPath> {
        let rect = frame.map_or(self.rect, native_rect);
        Edge::ALL
            .into_iter()
            .filter_map(|edge| {
                let style = self
                    .table
                    .style
                    .border
                    .as_ref()
                    .map(|border| edge.style(border))
                    .or(self.default_outer)?;
                Some(BorderPath {
                    edge,
                    style,
                    endpoints: edge.endpoints(rect, true),
                })
            })
            .collect()
    }

    pub fn outline(&self) -> OutlineStyle {
        let mut result = OutlineStyle::default();
        for path in self.outer_paths(None) {
            let style = path.style;
            if style.color != 0 && style.width > 0.0 {
                result.color = style.color;
                result.width = result.width.max(style.width.max(1.0));
            }
            let radius = style.start_radius.max(style.end_radius);
            match path.edge {
                Edge::Left => result.ry = radius,
                Edge::Top => result.rx = radius,
                Edge::Right => result.ry = result.ry.max(radius),
                Edge::Bottom => result.rx = result.rx.max(radius),
            }
        }
        result
    }
}

fn native_rect(rect: BoundingBox) -> [f32; 4] {
    [
        rect.x_min as f32,
        rect.y_min as f32,
        rect.x_max as f32,
        rect.y_max as f32,
    ]
}

fn prefix_ends(
    origin: f32,
    lengths: impl Iterator<Item = f32>,
) -> Result<Vec<f32>, ObjectDiagnosticKind> {
    let mut cursor = origin;
    lengths
        .map(|length| {
            if !length.is_finite() || length < 0.0 {
                return Err(ObjectDiagnosticKind::InvalidBounds);
            }
            cursor += length;
            if !cursor.is_finite() {
                return Err(ObjectDiagnosticKind::InvalidBounds);
            }
            Ok(cursor)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TableRecordMetadata;
    use crate::render::table::tests::grid;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    #[derive(Deserialize)]
    struct Capture {
        apk_version: String,
        apk_sha256: String,
        library_sha256: String,
        allocation_fills: Vec<u8>,
        cell_border_path_address: String,
        outer_border_path_address: String,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        native_defaults: bool,
        content_bbox: [f32; 4],
        heights: Vec<f32>,
        widths: Vec<f32>,
        spans: Vec<[u32; 2]>,
        default_border: Option<[CapturedStyle; 4]>,
        outer_border: Option<[CapturedStyle; 4]>,
        borders: Vec<Option<[CapturedStyle; 4]>>,
        paths: Vec<Vec<CapturedPath>>,
        outer_paths: Vec<CapturedPath>,
    }

    #[derive(Clone, Copy, Deserialize)]
    struct CapturedStyle {
        color: u32,
        width: f32,
        start_radius: f32,
        end_radius: f32,
    }

    impl From<CapturedStyle> for TableEdgeStyle {
        fn from(style: CapturedStyle) -> Self {
            Self {
                color: style.color,
                width: style.width,
                start_radius: style.start_radius,
                end_radius: style.end_radius,
            }
        }
    }

    #[derive(Deserialize)]
    struct CapturedPath {
        edge: u32,
        style: CapturedStyle,
        endpoints: [f32; 4],
        equation: [f32; 3],
    }

    fn border([left, top, right, bottom]: [TableEdgeStyle; 4]) -> TableBorder {
        TableBorder {
            left,
            top,
            right,
            bottom,
            metadata: TableRecordMetadata::default(),
        }
    }

    fn check_paths(actual: &[BorderPath], expected: &[CapturedPath], context: &str) {
        assert_eq!(actual.len(), expected.len(), "{context}");
        for (index, (path, captured)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(path.edge as u32, captured.edge, "{context}, path {index}");
            assert_eq!(
                path.style,
                TableEdgeStyle::from(captured.style),
                "{context}, path {index}"
            );
            assert_eq!(
                path.endpoints, captured.endpoints,
                "{context}, path {index}"
            );
            let [x1, y1, x2, y2] = path.endpoints;
            let equation = [y1 - y2, x2 - x1, x1.mul_add(y2, -(x2 * y1))];
            assert_eq!(equation, captured.equation, "{context}, equation {index}");
        }
        let mut scene = crate::render::vector::Scene::new(crate::render::vector::Svg::new());
        for &path in actual {
            crate::render::paint_table_border(
                &mut scene,
                path,
                0.0,
                crate::render::RenderTheme::for_canvas(false),
            );
        }
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let nodes: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("line"))
            .collect();
        let drawable: Vec<_> = expected
            .iter()
            .filter(|path| path.style.color != 0 && path.style.width != 0.0)
            .collect();
        assert_eq!(nodes.len(), drawable.len(), "{context}, vector paths");
        for (node, path) in nodes.iter().zip(drawable) {
            let coordinates = ["x1", "y1", "x2", "y2"]
                .map(|name| node.attribute(name).unwrap().parse::<f64>().unwrap());
            assert_eq!(
                coordinates,
                path.endpoints.map(f64::from),
                "{context}, vector coordinates"
            );
        }
    }

    #[test]
    fn native_perimeters_styles_and_float_staging_match() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-border-paths.json"
        ));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "4cae7223084cff60765bddb5e0b9926e8a706b741bf7cb9949cebfe4e2f8dc0d"
        );
        let capture: Capture = serde_json::from_slice(bytes).unwrap();
        assert_eq!(capture.apk_version, "4.4.45.37");
        assert_eq!(
            capture.apk_sha256,
            "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
        );
        assert_eq!(
            capture.library_sha256,
            "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
        );
        assert_eq!(capture.allocation_fills, [0, 165, 255]);
        assert_eq!(capture.cell_border_path_address, "0x3cad9c");
        assert_eq!(capture.outer_border_path_address, "0x3cb464");
        assert_eq!(capture.cases.len(), 12);
        let mut segment_count = 0;
        let mut outer_count = 0;
        for case in capture.cases {
            let mut table = grid(&case.heights, &case.widths);
            let [x_min, y_min, x_max, y_max] = case.content_bbox.map(f64::from);
            table.style.content_bbox = Some(BoundingBox {
                x_min,
                y_min,
                x_max,
                y_max,
            });
            table.style.default_cell_border = case
                .default_border
                .map(|styles| border(styles.map(Into::into)));
            table.style.border = case
                .outer_border
                .map(|styles| border(styles.map(Into::into)));
            for ((cell, span), styles) in table
                .rows
                .iter_mut()
                .flat_map(|row| &mut row.cells)
                .zip(case.spans)
                .zip(case.borders)
            {
                [cell.row_span, cell.column_span] = span;
                cell.border = styles.map(|styles| border(styles.map(Into::into)));
            }
            let topology = TableGrid::new(&table).unwrap();
            let defaults = case.native_defaults.then_some(DEFAULT_EDGE);
            let geometry =
                TableBorderGeometry::with_defaults(&table, &topology, defaults, defaults).unwrap();
            for (index, captured) in case.paths.iter().enumerate() {
                let position = CellPosition {
                    row: index / case.widths.len(),
                    column: index % case.widths.len(),
                };
                check_paths(
                    &geometry.cell_paths(position),
                    captured,
                    &format!("{}, cell {index}", case.name),
                );
                segment_count += captured.len();
            }
            check_paths(&geometry.outer_paths(None), &case.outer_paths, &case.name);
            outer_count += case.outer_paths.len();
        }
        assert_eq!(segment_count, 364);
        assert_eq!(outer_count, 8);
    }

    #[test]
    fn omitted_serialized_borders_retain_native_constructor_defaults() {
        let mut table = grid(&[10.0; 2], &[20.0; 2]);
        table.style.border = None;
        table.style.default_cell_border = None;
        let topology = TableGrid::new(&table).unwrap();
        let geometry = TableBorderGeometry::new(&table, &topology).unwrap();
        assert_eq!(geometry.outer_paths(None).len(), 4);
        assert_eq!(
            geometry.outline(),
            OutlineStyle {
                color: 0xff000000,
                width: 1.0,
                rx: 0.0,
                ry: 0.0
            }
        );
        assert!(
            geometry
                .cell_paths(CellPosition { row: 0, column: 0 })
                .iter()
                .all(|path| path.style == DEFAULT_EDGE)
        );
        assert!(
            geometry
                .cell_paths(CellPosition {
                    row: usize::MAX,
                    column: 0
                })
                .is_empty()
        );
        assert!(
            geometry
                .cell_paths(CellPosition { row: 0, column: 2 })
                .is_empty()
        );
    }

    #[test]
    fn nonfinite_geometry_and_styles_are_explicit_failures() {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for field in 0..5 {
                let mut table = grid(&[10.0], &[20.0]);
                table.style.border = Some(border([DEFAULT_EDGE; 4]));
                match field {
                    0 => table.rows[0].height = invalid,
                    1 => table.column_widths[0] = invalid,
                    2 => table.style.border.as_mut().unwrap().left.width = invalid,
                    3 => table.style.border.as_mut().unwrap().top.start_radius = invalid,
                    _ => table.style.border.as_mut().unwrap().right.end_radius = invalid,
                }
                let topology = TableGrid::new(&table).unwrap();
                assert!(matches!(
                    TableBorderGeometry::new(&table, &topology),
                    Err(ObjectDiagnosticKind::InvalidBounds)
                ));
            }
        }
    }

    #[test]
    fn paint_work_is_bounded_after_frame_owner_propagation() {
        let mut table = grid(&[10.0], &[20.0; 2048]);
        table.rows[0].cells[0].column_span = 1024;
        table.rows[0].cells[1].column_span = 2047;
        let topology = TableGrid::new(&table).unwrap();
        assert_eq!(topology.visible_cells().len(), 1025);
        assert!(matches!(
            TableBorderGeometry::new(&table, &topology),
            Err(ObjectDiagnosticKind::UnsupportedContent)
        ));
    }

    #[test]
    fn model_style_retention_is_distinct_from_drawing_width_selection() {
        let mut table = grid(&[10.0], &[20.0]);
        table.style.border = Some(border(
            [TableEdgeStyle {
                color: 0,
                ..DEFAULT_EDGE
            }; 4],
        ));
        table.rows[0].cells[0].border = Some(border([
            TableEdgeStyle {
                width: 0.0,
                ..DEFAULT_EDGE
            },
            TableEdgeStyle {
                width: -5.0,
                ..DEFAULT_EDGE
            },
            TableEdgeStyle {
                color: 0x00123456,
                width: 0.25,
                ..DEFAULT_EDGE
            },
            TableEdgeStyle {
                width: 6.0,
                ..DEFAULT_EDGE
            },
        ]));
        let topology = TableGrid::new(&table).unwrap();
        let geometry = TableBorderGeometry::new(&table, &topology).unwrap();
        assert!(!geometry.outline().active());
        let paths = geometry.cell_paths(CellPosition { row: 0, column: 0 });
        assert_eq!(paths.len(), 4);
        assert_eq!(
            paths
                .iter()
                .map(|path| path.paint_width())
                .collect::<Vec<_>>(),
            [None, Some(1.0), Some(1.0), Some(6.0)]
        );
        assert_eq!(paths[2].style.color >> 24, 0);
    }

    #[test]
    fn signed_radius_maxima_do_not_manufacture_rounding() {
        let mut table = grid(&[10.0], &[20.0]);
        table.style.border = Some(border([
            TableEdgeStyle {
                start_radius: -4.0,
                end_radius: -8.0,
                ..DEFAULT_EDGE
            },
            TableEdgeStyle {
                start_radius: -3.0,
                end_radius: -7.0,
                ..DEFAULT_EDGE
            },
            TableEdgeStyle {
                start_radius: -9.0,
                end_radius: -2.0,
                ..DEFAULT_EDGE
            },
            TableEdgeStyle {
                start_radius: -6.0,
                end_radius: -1.0,
                ..DEFAULT_EDGE
            },
        ]));
        let topology = TableGrid::new(&table).unwrap();
        let outline = TableBorderGeometry::new(&table, &topology)
            .unwrap()
            .outline();
        assert_eq!([outline.rx, outline.ry], [-1.0, -2.0]);
        assert!(!outline.rounded());
        assert!(outline.active());
    }
}
