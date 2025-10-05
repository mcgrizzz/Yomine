use crate::{
    core::Term,
    dictionary::frequency_manager::FrequencyManager,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Frequency,
    Chronological,
    SentenceCount,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn reversed(self) -> Self {
        match self {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SortState {
    pub field: Option<SortField>,
    pub direction: SortDirection,
}

impl SortState {
    pub fn new(field: Option<SortField>, direction: SortDirection) -> Self {
        Self { field, direction }
    }

    pub fn default_direction(field: SortField) -> SortDirection {
        match field {
            SortField::Frequency | SortField::Chronological => SortDirection::Ascending,
            SortField::SentenceCount => SortDirection::Descending,
        }
    }

    pub fn toggle_or_set(&mut self, field: SortField) {
        match self.field {
            Some(current) if current == field => {
                self.direction = self.direction.reversed();
            }
            _ => {
                self.field = Some(field);
                self.direction = Self::default_direction(field);
            }
        }
    }
}

impl Default for SortState {
    fn default() -> Self {
        Self { field: Some(SortField::Frequency), direction: SortDirection::Ascending }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SentenceSortMode {
    Time,
    Count,
}

impl Default for SentenceSortMode {
    fn default() -> Self {
        SentenceSortMode::Time
    }
}

pub fn sort_indices(
    indices: &mut [usize],
    terms: &[Term],
    field: SortField,
    direction: SortDirection,
    frequency_manager: Option<&FrequencyManager>,
) {
    indices.sort_unstable_by(|&lhs, &rhs| {
        let left = &terms[lhs];
        let right = &terms[rhs];

        let ordering = match field {
            SortField::Frequency => {
                let left_freq = weighted_frequency(left, frequency_manager);
                let right_freq = weighted_frequency(right, frequency_manager);
                left_freq.cmp(&right_freq)
            }
            SortField::Chronological => {
                let left_ord = left
                    .sentence_references
                    .iter()
                    .map(|(idx, _)| *idx)
                    .min()
                    .unwrap_or(usize::MAX);
                let right_ord = right
                    .sentence_references
                    .iter()
                    .map(|(idx, _)| *idx)
                    .min()
                    .unwrap_or(usize::MAX);
                left_ord.cmp(&right_ord)
            }
            SortField::SentenceCount => {
                let left_count = left.sentence_references.len();
                let right_count = right.sentence_references.len();
                left_count.cmp(&right_count)
            }
        };

        match direction {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        }
    });
}

pub fn weighted_frequency(term: &Term, frequency_manager: Option<&FrequencyManager>) -> u32 {
    if let Some(manager) = frequency_manager {
        manager.get_weighted_harmonic(&term.frequencies)
    } else {
        term.frequencies.get("HARMONIC").copied().unwrap_or(u32::MAX)
    }
}
