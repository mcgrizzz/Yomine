use eframe::egui::{
    self,
    RichText,
    Style,
    Ui,
};
use egui::{
    epaint::Shadow,
    style::{
        Selection,
        WidgetVisuals,
        Widgets,
    },
    Color32,
    Stroke,
    Visuals,
};

#[derive(Clone)]
pub struct Theme {
    dark: Option<ThemeDetails>,
    light: Option<ThemeDetails>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::tokyo()
    }
}

impl Theme {
    fn light(&self) -> Option<&ThemeDetails> {
        match &self.light {
            Some(light) => Some(light),
            _ => None,
        }
    }

    fn dark(&self) -> Option<&ThemeDetails> {
        match &self.dark {
            Some(dark) => Some(dark),
            _ => None,
        }
    }

    //These need to be changed eventually.
    pub fn bold(&self, content: &str) -> RichText {
        return RichText::new(content).color(self.dark().unwrap().orange);
    }

    pub fn heading(&self, content: &str) -> RichText {
        return RichText::new(content).color(self.dark().unwrap().purple);
    }

    pub fn red(&self) -> Color32 {
        return self.dark().unwrap().red;
    }

    pub fn orange(&self) -> Color32 {
        return self.dark().unwrap().orange;
    }

    pub fn yellow(&self) -> Color32 {
        return self.dark().unwrap().yellow;
    }

    pub fn green(&self) -> Color32 {
        return self.dark().unwrap().green;
    }

    pub fn purple(&self) -> Color32 {
        return self.dark().unwrap().purple;
    }

    pub fn blue(&self) -> Color32 {
        return self.dark().unwrap().cyan.linear_multiply(0.8);
    }

    pub fn cyan(&self) -> Color32 {
        return self.dark().unwrap().cyan;
    }

    pub fn tokyo() -> Self {
        Theme {
            dark: Some(ThemeDetails::tokyo_night_storm()),
            light: Some(ThemeDetails::tokyo_night_light()),
        }
    }

    pub fn dracula() -> Self {
        Theme { dark: Some(ThemeDetails::dracula()), light: Some(ThemeDetails::dracula_light()) }
    }
}

#[derive(Clone)]
pub struct ThemeDetails {
    background: Color32,
    foreground: Color32,
    selection: Color32,
    comment: Color32,
    red: Color32,
    orange: Color32,
    yellow: Color32,
    green: Color32,
    purple: Color32,
    cyan: Color32,
    pink: Color32,
    background_darker: Color32,
    background_dark: Color32,
    background_light: Color32,
    background_lighter: Color32,
}

impl ThemeDetails {
    //Colors from:
    //https://github.com/ShabbirHasan1/egui_dracula/blob/master/src/lib.rs
    fn dracula() -> Self {
        Self {
            background: Color32::from_rgb(0x28, 0x2a, 0x36),
            foreground: Color32::from_rgb(0xf8, 0xf8, 0xf2),
            selection: Color32::from_rgb(0x44, 0x47, 0x5a),
            comment: Color32::from_rgb(0x62, 0x72, 0xa4),
            red: Color32::from_rgb(0xff, 0x55, 0x55),
            orange: Color32::from_rgb(0xff, 0xb8, 0x6c),
            yellow: Color32::from_rgb(0xf1, 0xfa, 0x8c),
            green: Color32::from_rgb(0x50, 0xfa, 0x7b),
            purple: Color32::from_rgb(189, 147, 249),
            cyan: Color32::from_rgb(139, 233, 253),
            pink: Color32::from_rgb(255, 121, 198),
            background_darker: Color32::from_rgb(25, 26, 33),
            background_dark: Color32::from_rgb(33, 35, 53),
            background_light: Color32::from_rgb(52, 54, 66),
            background_lighter: Color32::from_rgb(66, 69, 80),
        }
    }

    fn dracula_light() -> Self {
        Self {
            background: Color32::from_rgb(248, 248, 242), // Light base background
            foreground: Color32::from_rgb(40, 42, 54),    // Darker text for contrast
            selection: Color32::from_rgb(200, 200, 220),  // Light pastel for selection highlight
            comment: Color32::from_rgb(120, 130, 160),    // Subdued blue-gray for comments
            red: Color32::from_rgb(200, 80, 80),          // Softer red for warnings
            orange: Color32::from_rgb(220, 150, 90),      // Muted orange for accents
            yellow: Color32::from_rgb(220, 230, 120),     // Gentle yellow for highlights
            green: Color32::from_rgb(80, 200, 120),       // Balanced green for success
            purple: Color32::from_rgb(150, 120, 220),     // Softer purple for accents
            cyan: Color32::from_rgb(80, 190, 230),        // Muted cyan for links or highlights
            pink: Color32::from_rgb(230, 130, 200),       // Gentle pink for special highlights
            background_darker: Color32::from_rgb(235, 235, 230), // Slightly darker light tone
            background_dark: Color32::from_rgb(245, 245, 240), // Darker base for depth
            background_light: Color32::from_rgb(255, 255, 250), // Brighter tone for elevated elements
            background_lighter: Color32::from_rgb(255, 255, 255), // Lightest tone for highlights
        }
    }

    fn tokyo_night_storm() -> Self {
        Self {
            background: Color32::from_rgb(23, 24, 38),
            foreground: Color32::from_rgb(204, 204, 204),
            selection: Color32::from_rgb(68, 71, 90),
            comment: Color32::from_rgb(98, 114, 164),
            red: Color32::from_rgb(255, 121, 121),
            orange: Color32::from_rgb(255, 161, 90),
            yellow: Color32::from_rgb(241, 250, 140),
            green: Color32::from_rgb(86, 209, 123),
            purple: Color32::from_rgb(189, 147, 249),
            cyan: Color32::from_rgb(97, 175, 239),
            pink: Color32::from_rgb(255, 85, 255),
            background_darker: Color32::from_rgb(19, 20, 32),
            background_dark: Color32::from_rgb(27, 29, 45),
            background_light: Color32::from_rgb(42, 44, 66),
            background_lighter: Color32::from_rgb(56, 58, 78),
        }
    }

    fn tokyo_night_light() -> Self {
        Self {
            background: Color32::from_rgb(240, 240, 250), // Light base background
            foreground: Color32::from_rgb(40, 40, 40),    // Darker text for contrast
            selection: Color32::from_rgb(200, 200, 230),  // Soft highlight color
            comment: Color32::from_rgb(150, 160, 200),    // Muted blue for comments
            red: Color32::from_rgb(200, 80, 80),          // Softer red for warnings
            orange: Color32::from_rgb(220, 140, 60),      // Muted orange for highlights
            yellow: Color32::from_rgb(220, 230, 100),     // Gentle yellow for emphasis
            green: Color32::from_rgb(80, 180, 100),       // Balanced green for success
            purple: Color32::from_rgb(150, 120, 200),     // Subdued purple for accents
            cyan: Color32::from_rgb(80, 160, 200),        // Muted cyan for links or highlights
            pink: Color32::from_rgb(200, 100, 200),       // Soft pink for special highlights
            background_darker: Color32::from_rgb(220, 220, 240), // Darker light tone
            background_dark: Color32::from_rgb(230, 230, 245), // Slightly darker than main background
            background_light: Color32::from_rgb(245, 245, 255), // Lighter tone for raised elements
            background_lighter: Color32::from_rgb(255, 255, 255), // Lightest tone for highlights
        }
    }
}

pub fn set_theme(ctx: &egui::Context, theme: Theme) {
    if let Some(dark) = theme.dark() {
        set_theme_variant(ctx, dark, true);
    }

    if let Some(light) = theme.light() {
        set_theme_variant(ctx, light, false);
    }
}

pub fn blend_colors(color_a: Color32, color_b: Color32, t: f32) -> Color32 {
    let blend_channel = |a: u8, b: u8| ((1.0 - t) * (a as f32) + t * (b as f32)).round() as u8;
    Color32::from_rgba_unmultiplied(
        blend_channel(color_a.r(), color_b.r()),
        blend_channel(color_a.g(), color_b.g()),
        blend_channel(color_a.b(), color_b.b()),
        blend_channel(color_a.a(), color_b.a()),
    )
}

fn set_theme_variant(ctx: &egui::Context, theme: &ThemeDetails, is_dark: bool) {
    let (default, variant) = match is_dark {
        true => (Visuals::dark(), egui::Theme::Dark),
        false => (Visuals::light(), egui::Theme::Light),
    };

    ctx.set_visuals_of(
        variant,
        Visuals {
            dark_mode: is_dark,
            widgets: Widgets {
                noninteractive: WidgetVisuals {
                    bg_fill: theme.background,
                    weak_bg_fill: theme.background_lighter,
                    bg_stroke: Stroke {
                        color: theme.background_dark,
                        ..default.widgets.noninteractive.bg_stroke
                    },
                    fg_stroke: Stroke {
                        color: theme.foreground,
                        ..default.widgets.noninteractive.fg_stroke
                    },
                    ..default.widgets.noninteractive
                },
                inactive: WidgetVisuals {
                    bg_fill: theme.background_light,
                    weak_bg_fill: theme.background_lighter,
                    bg_stroke: Stroke {
                        color: theme.background_dark,
                        ..default.widgets.inactive.bg_stroke
                    },
                    fg_stroke: Stroke {
                        color: theme.foreground,
                        ..default.widgets.inactive.fg_stroke
                    },
                    ..default.widgets.inactive
                },
                hovered: WidgetVisuals {
                    bg_fill: theme.selection,
                    weak_bg_fill: theme.background_lighter,
                    bg_stroke: Stroke { color: theme.cyan, ..default.widgets.hovered.bg_stroke },
                    fg_stroke: Stroke {
                        color: theme.foreground,
                        ..default.widgets.hovered.fg_stroke
                    },
                    ..default.widgets.hovered
                },
                active: WidgetVisuals {
                    bg_fill: theme.selection,
                    weak_bg_fill: theme.background_light,
                    bg_stroke: Stroke { color: theme.cyan, ..default.widgets.active.bg_stroke },
                    fg_stroke: Stroke {
                        color: theme.foreground,
                        ..default.widgets.active.fg_stroke
                    },
                    ..default.widgets.active
                },
                open: WidgetVisuals {
                    bg_fill: theme.background_dark,
                    weak_bg_fill: theme.background_lighter,
                    bg_stroke: Stroke { color: theme.purple, ..default.widgets.open.bg_stroke },
                    fg_stroke: Stroke { color: theme.foreground, ..default.widgets.open.fg_stroke },
                    ..default.widgets.open
                },
            },
            selection: Selection {
                bg_fill: theme.selection,
                stroke: Stroke { color: theme.foreground, ..default.selection.stroke },
            },
            hyperlink_color: theme.cyan,
            faint_bg_color: match is_dark {
                true => theme.background_darker,
                false => theme.background_light,
            },
            extreme_bg_color: theme.background_darker,
            code_bg_color: theme.background_dark,
            error_fg_color: theme.red,
            warn_fg_color: theme.orange,
            window_shadow: Shadow { color: theme.background_darker, ..default.window_shadow },
            window_fill: theme.background,
            window_stroke: Stroke { color: theme.background_light, ..default.window_stroke },
            panel_fill: theme.background_dark,
            popup_shadow: Shadow { color: theme.background_dark, ..default.popup_shadow },
            collapsing_header_frame: true,
            ..default
        },
    );

    ctx.all_styles_mut(|style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
}
