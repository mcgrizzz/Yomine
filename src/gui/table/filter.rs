use std::collections::HashMap;

use crate::segmentation::word::POS;

#[derive(Clone)]
pub(super) struct PosToggle {
    pos: POS,
    enabled: bool,
}

impl PosToggle {
    fn new(pos: POS) -> Self {
        Self { pos, enabled: true }
    }
}

#[derive(Clone)]
pub struct PosFilterState {
    toggles: Vec<PosToggle>,
}

impl PosFilterState {
    pub fn new() -> Self {
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

    pub fn is_enabled(&self, target: &POS) -> bool {
        self.toggles
            .iter()
            .find(|toggle| toggle.pos == *target)
            .map(|toggle| toggle.enabled)
            .unwrap_or(true)
    }

    pub fn snapshot_map(&self) -> HashMap<POS, bool> {
        self.toggles.iter().map(|toggle| (toggle.pos, toggle.enabled)).collect()
    }

    pub fn apply_snapshot_map(&mut self, snapshot: &HashMap<POS, bool>) {
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
    pub fn new() -> Self {
        Self {
            min_bound: 0,
            max_bound: 0,
            selected_min: 0,
            selected_max: 0,
            include_unknown: false,
        }
    }

    pub fn update_bounds(&mut self, min_bound: u32, max_bound: u32) {
        let max_bound = max_bound.max(min_bound);
        self.min_bound = min_bound;
        self.max_bound = max_bound;

        if self.selected_min == 0 && self.selected_max == 0 {
            self.selected_min = min_bound;
            self.selected_max = max_bound;
        } else {
            self.selected_min = self.selected_min.clamp(min_bound, max_bound);
            self.selected_max = self.selected_max.clamp(self.selected_min, max_bound);
        }
    }

    pub fn set_selected(&mut self, min_value: u32, max_value: u32) {
        let clamped_min = min_value.clamp(0, self.max_bound);
        let clamped_max = max_value.clamp(clamped_min, self.max_bound);
        self.selected_min = clamped_min;
        self.selected_max = clamped_max;
    }

    pub fn contains(&self, value: u32) -> bool {
        if value == u32::MAX {
            return self.include_unknown;
        }
        value >= self.selected_min && value <= self.selected_max
    }
}
