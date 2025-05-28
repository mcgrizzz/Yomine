use eframe::egui;

pub struct Modal<T> {
    pub open: bool,
    pub title: String,
    pub data: T,
    pub result: Option<ModalResult<T>>,
    pub config: ModalConfig,
}

/// Configuration for modal appearance and behavior
#[derive(Clone)]
pub struct ModalConfig {
    /// Whether the modal is resizable
    pub resizable: bool,
    /// Whether the modal has a collapsible title bar
    pub collapsible: bool,
    /// Fixed size of the modal (if any)
    pub fixed_size: Option<egui::Vec2>,
    /// Minimum size of the modal
    pub min_size: Option<egui::Vec2>,
    /// Whether to show a dark overlay behind the modal
    pub show_overlay: bool,
    /// Whether the modal should be centered
    pub centered: bool,
    /// Whether clicking outside the modal should close it
    pub close_on_outside_click: bool,
}

impl Default for ModalConfig {
    fn default() -> Self {
        Self {
            resizable: false,
            collapsible: false,
            fixed_size: None,
            min_size: Some(egui::Vec2::new(300.0, 200.0)),
            show_overlay: true,
            centered: true,
            close_on_outside_click: true,
        }
    }
}

#[derive(Debug)]
pub enum ModalResult<T> {
    Confirmed(T),
    Cancelled,
    Custom(String, T),
}

impl<T: Clone> Clone for ModalResult<T> {
    fn clone(&self) -> Self {
        match self {
            ModalResult::Confirmed(data) => ModalResult::Confirmed(data.clone()),
            ModalResult::Cancelled => ModalResult::Cancelled,
            ModalResult::Custom(name, data) => ModalResult::Custom(name.clone(), data.clone()),
        }
    }
}

impl<T: Default> Modal<T> {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            open: false,
            title: title.into(),
            data: T::default(),
            result: None,
            config: ModalConfig::default(),
        }
    }
}

impl<T> Modal<T> {
    pub fn new_with_data(title: impl Into<String>, data: T) -> Self {
        Self {
            open: false,
            title: title.into(),
            data,
            result: None,
            config: ModalConfig::default(),
        }
    }

    pub fn with_config(mut self, config: ModalConfig) -> Self {
        self.config = config;
        self
    }

    pub fn open(&mut self) {
        self.open = true;
        self.result = None;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn take_result(&mut self) -> Option<ModalResult<T>> {
        self.result.take()
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }

    pub fn show<F>(&mut self, ctx: &egui::Context, content: F) -> Option<ModalResult<T>>
    where
        F: FnOnce(&mut egui::Ui, &mut T) -> Option<ModalResult<T>>,
        T: Clone,
    {
        if !self.open {
            return None;
        }

        let mut result = None;
        let mut close_from_outside_click = false;

        if self.config.show_overlay {
            close_from_outside_click = self.show_overlay(ctx);
        }

        let mut window = egui::Window::new(&self.title)
            .collapsible(self.config.collapsible)
            .resizable(self.config.resizable);

        if let Some(size) = self.config.fixed_size {
            window = window.fixed_size(size);
        }

        if let Some(min_size) = self.config.min_size {
            window = window.min_size(min_size);
        }

        if self.config.centered {
            window = window.anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
        }

        window.show(ctx, |ui| {
            if let Some(modal_result) = content(ui, &mut self.data) {
                result = Some(modal_result.clone());
                self.result = Some(modal_result);

                match &result {
                    Some(ModalResult::Confirmed(_))
                    | Some(ModalResult::Cancelled)
                    | Some(ModalResult::Custom(_, _)) => {
                        self.open = false;
                    }
                    None => {}
                }
            }
        });

        if close_from_outside_click && self.config.close_on_outside_click {
            self.open = false;
            result = Some(ModalResult::Cancelled);
        }

        result
    }

    fn show_overlay(&self, ctx: &egui::Context) -> bool {
        let area_response = egui::Area::new(egui::Id::new("modal_overlay"))
            .order(egui::Order::Background)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let (_rect, response) =
                    ui.allocate_exact_size(screen_rect.size(), egui::Sense::click());
                ui.painter().rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(100));
                response.clicked()
            });

        area_response.inner
    }
}

pub fn action_buttons<T>(
    ui: &mut egui::Ui,
    data: &T,
    confirm_text: &str,
    cancel_text: &str,
) -> Option<ModalResult<T>>
where
    T: Clone,
{
    ui.horizontal(|ui| {
        if ui.button(confirm_text).clicked() {
            Some(ModalResult::Confirmed(data.clone()))
        } else if ui.button(cancel_text).clicked() {
            Some(ModalResult::Cancelled)
        } else {
            None
        }
    })
    .inner
}

pub fn confirmation_dialog(
    modal: &mut Modal<()>,
    ctx: &egui::Context,
    message: &str,
) -> Option<ModalResult<()>> {
    modal.show(ctx, |ui, _data| {
        ui.label(message);
        ui.add_space(10.0);
        action_buttons(ui, &(), "Yes", "No")
    })
}
