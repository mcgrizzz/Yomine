pub mod theme;
pub mod table;

use std::collections::HashSet;
use eframe::{egui, epaint::text::{FontInsert, InsertFontFamily}};
use table::{term_table, TableState};
use theme::{set_theme, Theme};

use crate::core::{Sentence, Term};

#[derive(Default)]
pub struct YomineApp {
    terms: Vec<Term>,
    sentences: Vec<Sentence>,
    table_state: TableState,
    zoom: f32,
    theme: Theme,
}



impl YomineApp {
    pub fn new(cc: &eframe::CreationContext<'_>, terms: Vec<Term>, sentences: Vec<Sentence>) -> Self {
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

        let mut terms: Vec<Term> = terms
            .into_iter()
            .filter(|term| seen.insert((term.lemma_form.clone(), term.lemma_reading.clone())))
            .collect();

        set_theme(&cc.egui_ctx, Theme::dracula());
        cc.egui_ctx.set_zoom_factor(cc.egui_ctx.zoom_factor() + 0.5);

        terms.sort_unstable_by(|a, b| {
            let freq_a = a.frequencies.get("HARMONIC").unwrap();
            let freq_b = b.frequencies.get("HARMONIC").unwrap();
            freq_a.cmp(freq_b)
        });
            
        Self {
            terms,
            sentences,
            table_state: TableState::default(),
            zoom: cc.egui_ctx.zoom_factor(),
            theme: Theme::dracula(),
        }
    }
    
}

impl eframe::App for YomineApp {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.top_bar(ctx);
        term_table(ctx, self);
    }
    
}

impl YomineApp {

    fn top_bar(&self, ctx: &egui::Context) {
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
    }
}

