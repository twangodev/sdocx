/// Units describe completed work within a stage, not elapsed processing time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ProgressStage {
    Archive,
    Resources,
    Pages,
    Objects,
    Layout,
    Preparing,
    Rendering,
    WritingPdf,
    Finalizing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Progress {
    pub stage: ProgressStage,
    pub completed: usize,
    pub total: Option<usize>,
}

impl Progress {
    pub fn new(stage: ProgressStage, completed: usize, total: Option<usize>) -> Self {
        Self {
            stage,
            completed,
            total,
        }
    }

    pub fn pending(stage: ProgressStage) -> Self {
        Self::new(stage, 0, None)
    }
}

pub(crate) struct WorkProgress<'a> {
    observer: &'a mut dyn FnMut(Progress),
    stage: ProgressStage,
    completed: usize,
    total: usize,
}

impl<'a> WorkProgress<'a> {
    pub(crate) fn new(
        observer: &'a mut dyn FnMut(Progress),
        stage: ProgressStage,
        total: usize,
    ) -> Self {
        observer(Progress::new(stage, 0, Some(total)));
        Self {
            observer,
            stage,
            completed: 0,
            total,
        }
    }

    pub(crate) fn advance(&mut self, count: usize) {
        self.completed += count;
        (self.observer)(Progress::new(self.stage, self.completed, Some(self.total)));
    }
}
