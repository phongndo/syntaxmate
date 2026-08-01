use super::state::StateId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCheckpoint {
    /// Source line index this state starts at.
    pub line_index: usize,
    pub state: StateId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointTable {
    interval: usize,
    checkpoints: Vec<LineCheckpoint>,
}

impl CheckpointTable {
    pub fn new(interval: usize) -> Self {
        let mut table = Self {
            interval: interval.max(1),
            checkpoints: Vec::new(),
        };
        table.record(0, StateId(0));
        table
    }

    pub fn interval(&self) -> usize {
        self.interval
    }

    pub fn checkpoints(&self) -> &[LineCheckpoint] {
        &self.checkpoints
    }

    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }

    pub fn nearest_before(&self, target_line: usize) -> Option<LineCheckpoint> {
        let end = self
            .checkpoints
            .partition_point(|checkpoint| checkpoint.line_index <= target_line);
        end.checked_sub(1)
            .and_then(|index| self.checkpoints.get(index))
            .copied()
    }

    pub fn record_if_boundary(&mut self, line_index: usize, state: StateId) {
        if line_index == 0 || line_index.is_multiple_of(self.interval) {
            self.record(line_index, state);
        }
    }

    pub fn record(&mut self, line_index: usize, state: StateId) {
        match self
            .checkpoints
            .binary_search_by_key(&line_index, |checkpoint| checkpoint.line_index)
        {
            Ok(index) => self.checkpoints[index].state = state,
            Err(index) => self
                .checkpoints
                .insert(index, LineCheckpoint { line_index, state }),
        }
    }

    pub fn invalidate_from(&mut self, line_index: usize) {
        self.checkpoints
            .retain(|checkpoint| checkpoint.line_index < line_index || checkpoint.line_index == 0);
    }
}
