pub mod theme;

use std::collections::HashSet;

use eframe::{egui::{self}, epaint::text::{FontInsert, InsertFontFamily}};
use egui_extras::{Column, TableBuilder};
use theme::{set_theme, Theme};

use crate::core::Term;

enum TableSort {
    FrequencyDescending,
    FrequencyAscending,
}

impl TableSort {
    fn text(&self) -> String {
        match &self {
            TableSort::FrequencyAscending => "⬆".to_string(),
            TableSort::FrequencyDescending => "⬇".to_string(),
        }
    }

    fn click(&self) -> Self {
        match &self {
            TableSort::FrequencyAscending => TableSort::FrequencyDescending,
            TableSort::FrequencyDescending => TableSort::FrequencyAscending,
        }
    }
}

impl Default for TableSort {
    fn default() -> Self {
        Self::FrequencyAscending
    }
}

#[derive(Default)]
pub struct YomineApp {
    terms: Vec<Term>,
    sort: TableSort,
    pos_english: bool,
    zoom: f32,
    theme: Theme,
}



impl YomineApp {
    pub fn new(cc: &eframe::CreationContext<'_>, terms: Vec<Term>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        cc.egui_ctx.add_font(FontInsert::new(
            "Noto Sans",
            egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/NotoSansJP-Regular.ttf"
            )),
            vec![
                InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: egui::epaint::text::FontPriority::Highest,
                },
                InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Lowest,
                },
            ],
        ));

        cc.egui_ctx.add_font(FontInsert::new(
            "Noto Sans",
            egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/NotoSansJP-Bold.ttf"
            )),
            vec![
                InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: egui::epaint::text::FontPriority::Highest,
                },
                InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Lowest,
                },
            ],
        ));
        
        let mut seen = HashSet::new();

        let terms = terms
            .into_iter()
            .filter(|term| seen.insert((term.lemma_form.clone(), term.lemma_reading.clone())))
            .collect();



        set_theme(&cc.egui_ctx, Theme::dracula());
        cc.egui_ctx.set_zoom_factor(cc.egui_ctx.zoom_factor() + 0.5);

        Self {
            terms,
            sort: TableSort::FrequencyAscending,
            zoom: cc.egui_ctx.zoom_factor(),
            theme: Theme::dracula(),
            pos_english: false,
        }
    }
    
}

impl eframe::App for YomineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        
        egui::CentralPanel::default().show(ctx, |ui| {
            let text_height = egui::TextStyle::Body
                .resolve(ui.style())
                .size
                .max(ui.spacing().interact_size.y);

            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("Term Table");
            egui::ScrollArea::vertical().show(ui, |ui| {
                TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto().at_least(70.0))
                .column(Column::auto().at_least(80.0))
                .column(Column::auto().at_least(40.0))
                .column(Column::remainder())
                .header(25.0, |mut header| {
                    header.col(|ui| {
                        ui.label(self.theme.heading("Term"));
                    });
                    header.col(|ui| {
                        ui.label(self.theme.heading("Reading"));
                    });
                    header.col(|ui| {
                        egui::Sides::new().height(25.0).show(
                            ui,
                            |ui| {
                                if ui.button(self.sort.text()).clicked() {
                                   self.sort = self.sort.click();
                                   self.terms.sort_unstable_by(|a, b| match self.sort {
                                    TableSort::FrequencyAscending => {
                                        let freq_a = a.frequencies.get("HARMONIC").unwrap();
                                        let freq_b = b.frequencies.get("HARMONIC").unwrap();
                                        freq_a.cmp(freq_b) // Ascending order
                                    }
                                    TableSort::FrequencyDescending => {
                                        let freq_a = a.frequencies.get("HARMONIC").unwrap();
                                        let freq_b = b.frequencies.get("HARMONIC").unwrap();
                                        freq_b.cmp(freq_a) // Descending order
                                    }
                                });
                                }
                            },
                            |ui| {
                                ui.label(self.theme.heading("Frequency"));
                            },
                        );
                        
                    });
                    header.col(|ui| {
                        egui::Sides::new().height(25.0).show(
                            ui,
                            |ui| {
                                let pos_text = match self.pos_english {
                                    true => "  ENG  ",
                                    false => "日本語",
                                };
                                if ui.button(pos_text).clicked() {
                                   self.pos_english = !self.pos_english;
                                }
                            },
                            |ui| {
                                ui.label(self.theme.heading("POS"));
                            },
                        );
                    });
                })
                .body(|mut body| {
                    body.rows(text_height, self.terms.len(), |mut row| {
                        let t = &self.terms[row.index()];
                        row.col(|ui| {
                            ui.strong(self.theme.bold(&t.lemma_form));
                        });
                        row.col(|ui| {
                            ui.label(&t.lemma_reading);
                        });
                        row.col(|ui| {
                            if let Some(&freq) = t.frequencies.get("HARMONIC") {
                                ui.label(if freq == u32::MAX { "？".to_string() } else { freq.to_string() });
                            }
                        });

                        let mut pos = t.part_of_speech.key.clone();
                        let pos_english = t.part_of_speech.english_name.clone();
                        let pos_hint = t.part_of_speech.hint.clone();
                        let pos_examples = t.part_of_speech.examples.clone();

                        if self.pos_english {
                            pos = pos_english.clone();
                        }

                        row.col(|ui| {
                            ui.label(pos)
                                .on_hover_ui(|ui| {
                                    ui.label(self.theme.heading(&pos_english));
                                    ui.label(pos_hint);
                                    ui.label(pos_examples.join(" 、"));
                                });
                        });
                    });
                });
            });
            
        });

    }
    
}

