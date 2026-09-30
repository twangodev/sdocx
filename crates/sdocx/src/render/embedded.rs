use super::{code::PreparedCode, table::PreparedTable};

pub(super) enum PreparedObject {
    Code(Box<PreparedCode>),
    Table(Box<PreparedTable>),
}

impl PreparedObject {
    pub fn height(&self) -> f64 {
        let bbox = match self {
            Self::Code(code) => code.panel_bbox,
            Self::Table(table) => table.measured_bbox,
        };
        bbox.y_max - bbox.y_min
    }

    pub fn minimum_first_page_height(&self) -> Option<f64> {
        match self {
            Self::Code(code) => code.minimum_first_page_height(),
            Self::Table(table) => table.minimum_first_page_height(),
        }
    }
}
