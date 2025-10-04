use std::collections::HashMap;

use eframe::egui;

use super::{
    filter::{
        FrequencyFilter,
        PosFilterState,
    },
    search,
    sort::{
        self,
        SentenceSortMode,
        SortDirection,
        SortField,
        SortState,
    },
};
use crate::{
    core::{
        Sentence,
        Term,
    },
    dictionary::frequency_manager::{
        DictionaryState,
        FrequencyManager,
    },
    segmentation::word::POS,
};

pub struct TableState {
    sort: SortState,
    sentence_sort_mode: SentenceSortMode,
    sentence_indices: HashMap<usize, usize>,
    visible_indices: Vec<usize>,
    dirty: bool,
    freq_filter: FrequencyFilter,
    pos_filters: PosFilterState,
    search: String,
    frequency_states: HashMap<String, DictionaryState>,
    term_column_width: Option<f32>,
}

impl Default for TableState {
    fn default() -> Self {
        Self {
            sort: SortState::default(),
            sentence_sort_mode: SentenceSortMode::default(),
            sentence_indices: HashMap::new(),
            visible_indices: Vec::new(),
            dirty: true,
            freq_filter: FrequencyFilter::new(),
            pos_filters: PosFilterState::new(),
            search: String::new(),
            frequency_states: HashMap::new(),
            term_column_width: None,
        }
    }
}

impl TableState {
    pub fn get_sentence_index(&self, term_index: usize) -> usize {
        self.sentence_indices.get(&term_index).copied().unwrap_or(0)
    }

    pub fn next_sentence(&mut self, term_index: usize, total_sentences: usize) {
        if total_sentences == 0 {
            return;
        }
        let current = self.get_sentence_index(term_index);
        let next = (current + 1) % total_sentences;
        self.sentence_indices.insert(term_index, next);
    }

    pub fn prev_sentence(&mut self, term_index: usize, total_sentences: usize) {
        if total_sentences == 0 {
            return;
        }
        let current = self.get_sentence_index(term_index);
        let next = if current == 0 { total_sentences - 1 } else { current - 1 };
        self.sentence_indices.insert(term_index, next);
    }

    pub fn reset(&mut self) {
        self.sentence_indices.clear();
        self.visible_indices.clear();
        self.freq_filter = FrequencyFilter::new();
        self.pos_filters = PosFilterState::new();
        self.term_column_width = None;
        self.dirty = true;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_search(&mut self, search: String) {
        if self.search != search {
            self.search = search;
            self.dirty = true;
            self.term_column_width = None;
        }
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    pub fn sort_state(&self) -> SortState {
        self.sort
    }

    pub fn sort_field(&self) -> Option<SortField> {
        self.sort.field
    }

    pub fn apply_pos_settings(&mut self, settings: &HashMap<String, bool>) {
        if settings.is_empty() {
            return;
        }
        let mut snapshot = HashMap::new();
        for (key, enabled) in settings {
            if let Some(pos) = POS::from_key(key) {
                snapshot.insert(pos, *enabled);
            }
        }

        if !snapshot.is_empty() {
            self.pos_filters.apply_snapshot_map(&snapshot);
            self.mark_dirty();
            self.term_column_width = None; // Recalculate width when filters change
        }
    }

    pub fn export_pos_settings(&self) -> HashMap<String, bool> {
        self.pos_filters
            .snapshot_map()
            .into_iter()
            .map(|(pos, enabled)| (pos.as_key().to_string(), enabled))
            .collect()
    }

    pub fn pos_snapshot(&self) -> HashMap<POS, bool> {
        self.pos_filters.snapshot_map()
    }

    pub fn sentence_sort_mode(&self) -> SentenceSortMode {
        self.sentence_sort_mode
    }

    pub fn set_sort(&mut self, field: SortField, direction: SortDirection) {
        if self.sort.field != Some(field) || self.sort.direction != direction {
            self.sort = SortState::new(Some(field), direction);
            match field {
                SortField::Chronological => self.sentence_sort_mode = SentenceSortMode::Time,
                SortField::SentenceCount => self.sentence_sort_mode = SentenceSortMode::Count,
                _ => {}
            }
            self.dirty = true;
        }
    }

    pub fn clear_sort(&mut self) {
        if self.sort.field.is_some() {
            self.sort = SortState::new(None, SortDirection::Ascending);
            self.dirty = true;
        }
    }

    pub fn set_sentence_sort_mode(&mut self, mode: SentenceSortMode) {
        if self.sentence_sort_mode != mode
            || !matches!(self.sort.field, Some(SortField::Chronological | SortField::SentenceCount))
        {
            self.sentence_sort_mode = mode;
            let target_field = match mode {
                SentenceSortMode::Time => SortField::Chronological,
                SentenceSortMode::Count => SortField::SentenceCount,
            };
            let direction = if self.sort.field == Some(target_field) {
                self.sort.direction
            } else {
                SortState::default_direction(target_field)
            };
            self.set_sort(target_field, direction);
        }
    }

    pub fn frequency_filter(&self) -> FrequencyFilter {
        self.freq_filter
    }

    pub fn set_frequency_range(&mut self, min_value: u32, max_value: u32) {
        self.freq_filter.set_selected(min_value, max_value);
        self.dirty = true;
        self.term_column_width = None; // Recalculate width when filters change
    }

    pub fn set_include_unknown(&mut self, include: bool) {
        if self.freq_filter.include_unknown != include {
            self.freq_filter.include_unknown = include;
            self.dirty = true;
            self.term_column_width = None; // Recalculate width when filters change
        }
    }

    pub fn sync_frequency_states(&mut self, frequency_manager: Option<&FrequencyManager>) {
        if let Some(manager) = frequency_manager {
            if let Some(states) = manager.dictionary_states() {
                self.frequency_states = states;
            }
        }
    }

    pub fn frequency_states(&self) -> &HashMap<String, DictionaryState> {
        &self.frequency_states
    }

    pub fn update_frequency_state(&mut self, name: String, state: DictionaryState) {
        self.frequency_states.insert(name, state);
        self.dirty = true;
    }

    pub fn ensure_indices(
        &mut self,
        terms: &[Term],
        sentences: &[Sentence],
        frequency_manager: Option<&FrequencyManager>,
    ) {
        let needs_rebuild = self.dirty
            || self.visible_indices.len() > terms.len()
            || self.visible_indices.iter().any(|&idx| idx >= terms.len());

        if needs_rebuild {
            self.recompute_indices(terms, sentences, frequency_manager);
        }
    }

    pub fn visible_indices(&self) -> &[usize] {
        &self.visible_indices
    }

    pub fn compute_term_column_width(&mut self, ctx: &egui::Context, terms: &[Term]) {
        if self.term_column_width.is_some() {
            return;
        }

        let mut max_width: f32 = 100.0; // Minimum width
        let fonts = ctx.fonts(|f| f.clone());

        for &term_index in &self.visible_indices {
            if let Some(term) = terms.get(term_index) {
                let term_width = fonts
                    .layout_no_wrap(
                        term.lemma_form.clone(),
                        egui::FontId::proportional(22.0), // Match the actual rendering size in col_term
                        egui::Color32::WHITE,
                    )
                    .size()
                    .x;

                let total_width = term_width + 30.0; // 30.0 for padding
                max_width = max_width.max(total_width);
            }
        }

        self.term_column_width = Some(max_width.min(300.0)); // Cap at 300px
    }

    pub fn term_column_width(&self) -> f32 {
        self.term_column_width.unwrap_or(100.0)
    }

    pub fn configure_bounds(&mut self, terms: &[Term]) {
        let mut min_freq = u32::MAX;
        let mut max_freq = 0;
        for term in terms {
            if let Some(freq) = term.frequencies.get("HARMONIC").copied() {
                if freq == u32::MAX {
                    continue;
                }
                min_freq = min_freq.min(freq);
                max_freq = max_freq.max(freq);
            }
        }

        if min_freq == u32::MAX {
            min_freq = 0;
        }
        if max_freq == 0 {
            max_freq = min_freq.max(1);
        }

        self.freq_filter.update_bounds(min_freq, max_freq);
        self.dirty = true;
    }

    fn recompute_indices(
        &mut self,
        terms: &[Term],
        sentences: &[Sentence],
        frequency_manager: Option<&FrequencyManager>,
    ) {
        if self.freq_filter.max_bound == 0 && !terms.is_empty() {
            self.configure_bounds(terms);
        }

        self.visible_indices.clear();

        let query = self.search.trim();

        for (idx, term) in terms.iter().enumerate() {
            if !self.term_matches(term, sentences, frequency_manager, query) {
                continue;
            }
            self.visible_indices.push(idx);
        }

        if let Some(field) = self.sort.field {
            sort::sort_indices(
                &mut self.visible_indices,
                terms,
                field,
                self.sort.direction,
                frequency_manager,
            );
        }

        self.dirty = false;
    }

    fn term_matches(
        &self,
        term: &Term,
        sentences: &[Sentence],
        frequency_manager: Option<&FrequencyManager>,
        query: &str,
    ) -> bool {
        if !self.freq_filter.contains(sort::weighted_frequency(term, frequency_manager)) {
            return false;
        }

        if !self.pos_filters.is_enabled(&term.part_of_speech) {
            return false;
        }

        search::matches_search(term, sentences, query)
    }
}
