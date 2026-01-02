use eframe::egui;
use crate::core::media_server::SubtitleTrack;

pub struct SubtitleSelectModal {
    is_open: bool,
    tracks: Vec<SubtitleTrack>,
    selected_track: Option<SubtitleTrack>,
}

impl Default for SubtitleSelectModal {
    fn default() -> Self {
        Self {
            is_open: false,
            tracks: Vec::new(),
            selected_track: None,
        }
    }
}

impl SubtitleSelectModal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, tracks: Vec<SubtitleTrack>) {
        self.tracks = tracks;
        self.is_open = true;
        self.selected_track = None;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<SubtitleTrack> {
        if !self.is_open {
            return None;
        }

        let mut result = None;
        let mut should_close = false;

        egui::Modal::new(egui::Id::new("subtitle_select_modal")).show(ctx, |ui| {
            ui.set_max_width(500.0);
            ui.set_max_height(400.0);
            
            ui.heading("Select Subtitle Track");
            ui.separator();

            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                 for track in &self.tracks {
                     let label = format!(
                        "{} - {} [{}] {}", 
                        track.language, 
                        track.title, 
                        track.codec,
                        if track.is_default { "(Default)" } else { "" }
                     );
                     
                     if ui.button(label).clicked() {
                         result = Some(track.clone());
                         should_close = true;
                     }
                 }
            });

            ui.add_space(10.0);
            
            if ui.button("Cancel").clicked() {
                should_close = true;
            }
        });

        if should_close {
            self.is_open = false;
        }

        result
    }
}
