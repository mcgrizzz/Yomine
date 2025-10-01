use std::collections::HashMap;

use wana_kana::ConvertJapanese;

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Frequency,
    Chronological,
    SentenceCount,
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
    fn new(field: Option<SortField>, direction: SortDirection) -> Self {
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

#[derive(Clone)]
pub(crate) struct PosToggle {
    pos: POS,
    enabled: bool,
}

impl PosToggle {
    fn new(pos: POS) -> Self {
        Self { pos, enabled: true }
    }
}

#[derive(Clone)]
pub(crate) struct PosFilterState {
    toggles: Vec<PosToggle>,
}

impl PosFilterState {
    fn new() -> Self {
        use POS::*;
        let toggles = vec![
            PosToggle::new(Noun),
            PosToggle::new(ProperNoun),
            PosToggle::new(CompoundNoun),
            PosToggle::new(Pronoun),
            PosToggle::new(Adjective),
            PosToggle::new(AdjectivalNoun),
            PosToggle::new(Adverb),
            PosToggle::new(Determiner),
            PosToggle::new(Preposition),
            PosToggle::new(Postposition),
            PosToggle::new(Verb),
            PosToggle::new(SuruVerb),
            PosToggle::new(Copula),
            PosToggle::new(Suffix),
            PosToggle::new(Prefix),
            PosToggle::new(Conjunction),
            PosToggle::new(Interjection),
            PosToggle::new(Number),
            PosToggle::new(Counter),
            PosToggle::new(Symbol),
            PosToggle::new(Expression),
            PosToggle::new(NounExpression),
            PosToggle::new(Other),
            PosToggle::new(Unknown),
        ];
        Self { toggles }
    }

    fn is_enabled(&self, target: &POS) -> bool {
        self.toggles
            .iter()
            .find(|toggle| toggle.pos == *target)
            .map(|toggle| toggle.enabled)
            .unwrap_or(true)
    }

    pub(crate) fn snapshot_map(&self) -> HashMap<POS, bool> {
        self.toggles.iter().map(|toggle| (toggle.pos, toggle.enabled)).collect()
    }

    pub(crate) fn apply_snapshot_map(&mut self, snapshot: &HashMap<POS, bool>) {
        for toggle in &mut self.toggles {
            if let Some(enabled) = snapshot.get(&toggle.pos) {
                toggle.enabled = *enabled;
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct FrequencyFilter {
    pub min_bound: u32,
    pub max_bound: u32,
    pub selected_min: u32,
    pub selected_max: u32,
    pub include_unknown: bool,
}

impl FrequencyFilter {
    fn new() -> Self {
        Self {
            min_bound: 0,
            max_bound: 0,
            selected_min: 0,
            selected_max: 0,
            include_unknown: false,
        }
    }

    fn update_bounds(&mut self, min_bound: u32, max_bound: u32) {
        let max_bound = max_bound.max(min_bound);
        self.min_bound = min_bound;
        self.max_bound = max_bound;
        self.selected_min = min_bound;
        self.selected_max = max_bound;
    }

    pub fn set_selected(&mut self, min_value: u32, max_value: u32) {
        let clamped_min = min_value.clamp(0, self.max_bound);
        let clamped_max = max_value.clamp(clamped_min, self.max_bound);
        self.selected_min = clamped_min;
        self.selected_max = clamped_max;
    }

    fn contains(&self, value: u32) -> bool {
        if value == u32::MAX {
            return self.include_unknown;
        }
        value >= self.selected_min && value <= self.selected_max
    }
}

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
        self.dirty = true;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_search(&mut self, search: String) {
        if self.search != search {
            self.search = search;
            self.dirty = true;
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
    }

    pub fn set_include_unknown(&mut self, include: bool) {
        if self.freq_filter.include_unknown != include {
            self.freq_filter.include_unknown = include;
            self.dirty = true;
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

        let trimmed = self.search.trim().to_string();
        let query_lower = trimmed.to_lowercase();
        let query_hiragana = trimmed.to_hiragana();
        let query_hiragana_lower = query_hiragana.to_lowercase();

        for (idx, term) in terms.iter().enumerate() {
            if !self.term_matches(
                term,
                sentences,
                frequency_manager,
                &query_lower,
                &query_hiragana,
                &query_hiragana_lower,
            ) {
                continue;
            }
            self.visible_indices.push(idx);
        }

        self.sort_indices(terms, sentences, frequency_manager);
        self.dirty = false;
    }

    fn term_matches(
        &self,
        term: &Term,
        sentences: &[Sentence],
        frequency_manager: Option<&FrequencyManager>,
        query_lower: &str,
        query_hiragana: &str,
        query_hiragana_lower: &str,
    ) -> bool {
        if !self.freq_filter.contains(weighted_frequency(term, frequency_manager)) {
            return false;
        }

        if !self.pos_filters.is_enabled(&term.part_of_speech) {
            return false;
        }

        if query_lower.is_empty() && query_hiragana.trim().is_empty() {
            return true;
        }

        self.matches_search(term, sentences, query_lower, query_hiragana, query_hiragana_lower)
    }

    fn matches_search(
        &self,
        term: &Term,
        sentences: &[Sentence],
        query_lower: &str,
        query_hiragana: &str,
        query_hiragana_lower: &str,
    ) -> bool {
        let mut candidates = Vec::<String>::new();
        candidates.push(term.lemma_form.to_lowercase());
        candidates.push(term.surface_form.to_lowercase());
        candidates.push(term.full_segment.to_lowercase());
        candidates.push(term.lemma_reading.to_hiragana().to_lowercase());
        candidates.push(term.surface_reading.to_hiragana().to_lowercase());
        candidates.push(term.lemma_reading.to_katakana().to_lowercase());
        candidates.push(term.surface_reading.to_katakana().to_lowercase());
        candidates.push(term.part_of_speech.to_string().to_lowercase());

        let has_hiragana_query = !query_hiragana.trim().is_empty();

        if has_hiragana_query {
            candidates.push(term.lemma_form.to_hiragana().to_lowercase());
            candidates.push(term.surface_form.to_hiragana().to_lowercase());
        }

        if !query_lower.is_empty() && candidates.iter().any(|value| value.contains(query_lower)) {
            return true;
        }

        if has_hiragana_query && candidates.iter().any(|value| value.contains(query_hiragana_lower))
        {
            return true;
        }

        for (sentence_idx, _) in &term.sentence_references {
            if let Some(sentence) = sentences.get(*sentence_idx) {
                let text_lower = sentence.text.to_lowercase();
                if !query_lower.is_empty() && text_lower.contains(query_lower) {
                    return true;
                }
                if has_hiragana_query
                    && sentence.text.to_hiragana().to_lowercase().contains(query_hiragana_lower)
                {
                    return true;
                }
            }
        }

        false
    }

    fn sort_indices(
        &mut self,
        terms: &[Term],
        _sentences: &[Sentence],
        frequency_manager: Option<&FrequencyManager>,
    ) {
        let sort_state = self.sort;
        let Some(field) = sort_state.field else {
            return;
        };
        let direction = sort_state.direction;

        self.visible_indices.sort_unstable_by(|&lhs, &rhs| {
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
}

fn weighted_frequency(term: &Term, frequency_manager: Option<&FrequencyManager>) -> u32 {
    if let Some(manager) = frequency_manager {
        manager.get_weighted_harmonic(&term.frequencies)
    } else {
        term.frequencies.get("HARMONIC").copied().unwrap_or(u32::MAX)
    }
}
