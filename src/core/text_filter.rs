//! User-configurable text filters (issue #92): toggleable presets for common
//! Japanese subtitle noise plus custom regex→replacement rules, applied per
//! line before tokenization.

use regex::Regex;

use super::settings::SettingsData;

pub struct FilterPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub pattern: &'static str,
}

pub fn presets() -> &'static [FilterPreset] {
    &[
        FilterPreset {
            id: "parentheses",
            label: "Remove （…） text",
            description:
                "Speaker names and sound descriptions like （田中） or （ スマホ の 着信音 ）",
            pattern: r"[（(][^（）()]*[）)]",
        },
        FilterPreset {
            id: "lyric-lines",
            label: "Drop ♪ lyric lines",
            description: "Drops lines containing the song markers ♪ ♫ ♬",
            pattern: r"^.*[♪♫♬].*$",
        },
        FilterPreset {
            id: "lenticular",
            label: "Remove 【…】 text",
            description: "On-screen text and announcements like 【次回予告】",
            pattern: r"【[^【】]*】",
        },
        FilterPreset {
            id: "angle",
            label: "Remove 《…》 text",
            description: "Off-screen narration or radio-voice markers in some sources",
            pattern: r"《[^《》]*》",
        },
    ]
}

pub struct CompiledFilter {
    regex: Regex,
    replacement: String,
}

impl CompiledFilter {
    pub fn new(pattern: &str, replacement: &str) -> Result<Self, regex::Error> {
        Ok(Self { regex: Regex::new(pattern)?, replacement: replacement.to_string() })
    }
}

pub fn compile_filters(settings: &SettingsData) -> Vec<CompiledFilter> {
    let mut filters = Vec::new();
    for preset in presets() {
        if settings.text_filter_presets.get(preset.id).copied().unwrap_or(false) {
            filters
                .push(CompiledFilter::new(preset.pattern, "").expect("preset patterns are valid"));
        }
    }
    for rule in &settings.text_filters {
        if !rule.enabled {
            continue;
        }
        match CompiledFilter::new(&rule.pattern, &rule.replacement) {
            Ok(filter) => filters.push(filter),
            Err(e) => eprintln!("Skipping invalid text filter \"{}\": {}", rule.pattern, e),
        }
    }
    filters
}

pub fn apply_to_text(filters: &[CompiledFilter], text: &str) -> String {
    let mut out = text.to_string();
    for filter in filters {
        out = filter.regex.replace_all(&out, filter.replacement.as_str()).into_owned();
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::settings::TextFilterSetting;

    fn preset_only(id: &str) -> Vec<CompiledFilter> {
        let mut settings = SettingsData::default();
        settings.text_filter_presets.insert(id.to_string(), true);
        compile_filters(&settings)
    }

    #[test]
    fn all_preset_patterns_compile() {
        for preset in presets() {
            assert!(Regex::new(preset.pattern).is_ok(), "preset {} is invalid", preset.id);
        }
    }

    #[test]
    fn parentheses_preset_strips_names_and_drops_sfx_lines() {
        let filters = preset_only("parentheses");
        assert_eq!(apply_to_text(&filters, "（田中）こんにちは"), "こんにちは");
        assert_eq!(apply_to_text(&filters, "（ スマホ の 着信音 ）"), "");
        assert_eq!(apply_to_text(&filters, "(山田)おはよう"), "おはよう");
        assert_eq!(apply_to_text(&filters, "括弧なしの文"), "括弧なしの文");
    }

    #[test]
    fn lyric_preset_drops_marked_lines_only() {
        let filters = preset_only("lyric-lines");
        assert_eq!(apply_to_text(&filters, "♪ 夢の中へ"), "");
        assert_eq!(apply_to_text(&filters, "普通のセリフ"), "普通のセリフ");
    }

    #[test]
    fn bracket_presets_strip_their_pair() {
        assert_eq!(apply_to_text(&preset_only("lenticular"), "【次回予告】続きへ"), "続きへ");
        assert_eq!(apply_to_text(&preset_only("angle"), "《ラジオ》ニュースです"), "ニュースです");
    }

    #[test]
    fn custom_rules_apply_in_order_with_replacement() {
        let settings = SettingsData {
            text_filters: vec![
                TextFilterSetting {
                    pattern: "〜".into(), replacement: "ー".into(), enabled: true
                },
                TextFilterSetting { pattern: "A".into(), replacement: "".into(), enabled: false },
            ],
            ..Default::default()
        };
        let filters = compile_filters(&settings);
        assert_eq!(filters.len(), 1);
        assert_eq!(apply_to_text(&filters, "そ〜だA"), "そーだA");
    }

    #[test]
    fn invalid_custom_pattern_is_skipped() {
        let settings = SettingsData {
            text_filters: vec![TextFilterSetting {
                pattern: "[unclosed".into(),
                replacement: "".into(),
                enabled: true,
            }],
            ..Default::default()
        };
        assert!(compile_filters(&settings).is_empty());
    }
}
