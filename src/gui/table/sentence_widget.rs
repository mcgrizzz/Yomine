use eframe::egui::{
    self,
    Color32,
    Context,
    Ui,
    Widget,
};
use wana_kana::ConvertJapanese;

use crate::{
    core::Term,
    gui::{
        theme::blend_colors,
        YomineApp,
    },
    segmentation::word::POS,
};

pub struct SentenceWidget<'a> {
    segments: Vec<SegmentData>,
    surface_index: usize,
    is_expression: bool,
    term_text: String,
    segments_to_highlight: Vec<usize>,
    highlighted_color: Color32,
    normal_color: Color32,
    highlight_color: Color32,
    ctx: &'a Context,
    app: &'a YomineApp,
}

struct SegmentData {
    text: String,
    reading: String,
    pos: POS,
    start: usize,
    stop: usize,
    idx: usize,
}

impl<'a> SentenceWidget<'a> {
    pub fn new(
        ctx: &'a Context,
        term: &Term,
        app: &'a YomineApp,
        sentence_text: &'a str,
        segments: &[(String, POS, usize, usize)],
        surface_index: usize,
    ) -> Self {
        let highlighted_color = app.theme.red(ctx);
        let normal_color = ctx.style().visuals.widgets.noninteractive.fg_stroke.color;
        let highlight_color = ctx.style().visuals.widgets.noninteractive.bg_stroke.color;
        let is_expression = matches!(term.part_of_speech, POS::Expression | POS::NounExpression);

        let term_text =
            if is_expression { term.full_segment.clone() } else { term.surface_form.clone() };

        let segments_to_highlight = if is_expression {
            find_expression_segments(&term_text, segments, sentence_text)
        } else {
            Vec::new()
        };

        let segment_data: Vec<SegmentData> = segments
            .iter()
            .enumerate()
            .map(|(idx, (reading, pos, start, stop))| SegmentData {
                text: sentence_text[*start..*stop].to_string(),
                reading: reading.clone(),
                pos: pos.clone(),
                start: *start,
                stop: *stop,
                idx,
            })
            .collect();

        Self {
            segments: segment_data,
            surface_index,
            is_expression,
            term_text,
            segments_to_highlight,
            highlighted_color,
            normal_color,
            highlight_color,
            ctx,
            app,
        }
    }
}

impl<'a> Widget for SentenceWidget<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0; // No spacing between segments

            for seg in &self.segments {
                let is_term = self.is_segment_part_of_term(seg);

                let text_color = if is_term {
                    blend_colors(self.highlight_color, self.highlighted_color, 0.85)
                } else {
                    let pos_color = self.app.theme.pos_color(&seg.pos, self.ctx, self.normal_color);
                    let color = blend_colors(self.normal_color, pos_color, 0.75);
                    blend_colors(self.normal_color, color, 0.75)
                };

                let label =
                    egui::Label::new(egui::RichText::new(&seg.text).color(text_color).size(16.0));

                let reading_hiragana = seg.reading.to_hiragana();
                ui.add(label)
                    .on_hover_text(egui::RichText::new(&reading_hiragana).color(text_color));
            }
        })
        .response
    }
}

impl<'a> SentenceWidget<'a> {
    fn is_segment_part_of_term(&self, seg: &SegmentData) -> bool {
        if self.is_expression {
            self.segments_to_highlight.contains(&seg.idx)
        } else {
            let term_start = self.surface_index;
            let term_end = term_start + self.term_text.len();
            seg.start < term_end && seg.stop > term_start
        }
    }
}

fn find_expression_segments(
    term_text: &str,
    sentence_segments: &[(String, POS, usize, usize)],
    sentence_text: &str,
) -> Vec<usize> {
    let mut segments_to_highlight = Vec::new();
    let mut current_text = String::new();
    let mut start_idx = None;

    for (idx, (_, _, start, stop)) in sentence_segments.iter().enumerate() {
        let segment_text = &sentence_text[*start..*stop];
        let potential_text = current_text.clone() + segment_text;

        if term_text.starts_with(&potential_text) {
            if start_idx.is_none() {
                start_idx = Some(idx);
            }
            current_text = potential_text;

            if current_text == *term_text {
                if let Some(start) = start_idx {
                    for i in start..=idx {
                        segments_to_highlight.push(i);
                    }
                }
                break;
            }
        } else {
            current_text.clear();
            start_idx = None;

            if term_text.starts_with(segment_text) {
                current_text = segment_text.to_string();
                start_idx = Some(idx);

                if current_text == *term_text {
                    segments_to_highlight.push(idx);
                    break;
                }
            }
        }
    }

    segments_to_highlight
}
