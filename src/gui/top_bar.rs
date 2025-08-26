use std::{
    path::Path,
    process::Command,
    sync::{
        Arc,
        Mutex,
    },
};

use eframe::egui;

use crate::{
    core::{
        tasks::TaskManager,
        IgnoreList,
    },
    dictionary::frequency_utils,
    gui::{
        file_modal::FileModal,
        settings::{
            AnkiSettingsModal,
            IgnoreListModal,
            SettingsData,
            WebSocketSettingsModal,
        },
        websocket_manager::WebSocketManager,
    },
    persistence::get_app_data_dir,
    websocket::ServerState,
};

pub struct TopBar;

impl TopBar {
    pub fn show(
        ctx: &egui::Context,
        file_modal: &mut FileModal,
        anki_settings_modal: &mut AnkiSettingsModal,
        websocket_settings_modal: &mut WebSocketSettingsModal,
        ignore_list_modal: &mut IgnoreListModal,
        current_settings: &SettingsData,
        websocket_manager: &WebSocketManager,
        mpv_connected: bool,
        anki_connected: bool,
        restart_modal: &mut crate::gui::restart_modal::RestartModal,
        ignore_list: Option<&Arc<Mutex<IgnoreList>>>,
        task_manager: &TaskManager,
        can_refresh: bool,
    ) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                egui::widgets::global_theme_preference_switch(ui);
                ui.menu_button("File", |ui| {
                    if ui.button("Open New File").clicked() {
                        file_modal.open_dialog();
                    }

                    if ui.button("Load New Frequency Dictionaries").clicked() {
                        match frequency_utils::handle_frequency_dictionary_copy() {
                            Ok(count) => {
                                if count > 0 {
                                    println!("Successfully added {} frequency dictionaries", count);
                                    restart_modal.show_restart_dialog(format!(
                                        "Successfully added {} frequency dictionaries. \
                                         Please restart the application load them.",
                                        count
                                    ));
                                } else {
                                    println!(
                                        "No new frequency dictionaries were selected or loaded"
                                    );
                                    restart_modal.show_info_dialog(
                                        "No new frequency dictionaries were selected. \
                                         No changes were made.",
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to load frequency dictionaries: {}", e);
                                restart_modal.show_info_dialog(format!(
                                    "Failed to load frequency dictionaries: {}",
                                    e
                                ));
                            }
                        }
                    }

                    if ui.button("Open Data Folder").clicked() {
                        let data_dir = get_app_data_dir();
                        if let Err(e) = open_folder(&data_dir) {
                            eprintln!("Failed to open data directory: {}", e);
                        }
                    }

                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Settings", |ui| {
                    if ui.button("Anki").clicked() {
                        anki_settings_modal.open_settings(current_settings.clone(), ctx);
                    }
                    if ui.button("WebSocket Server").clicked() {
                        websocket_settings_modal.open_settings(current_settings.clone());
                    }
                    if ui.button("Ignore List").clicked() {
                        if let Some(ignore_list) = ignore_list {
                            ignore_list_modal.open_modal(ignore_list);
                        }
                    }
                });

                if can_refresh {
                    ui.separator();
                    let clicked = ui
                        .button("↻ Refresh")
                        .on_hover_ui(|ui| {
                            let cmds =
                                if cfg!(target_os = "macos") { "F5, Cmd+R" } else { "F5, Ctrl+R" };
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("Reapply ignorelist and Anki filters")
                                        .strong(),
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("Shortcut:")
                                            .italics()
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                    ui.label(
                                        egui::RichText::new(cmds)
                                            .monospace()
                                            .color(egui::Color32::from_rgb(180, 220, 255)),
                                    );
                                });
                            });
                        })
                        .clicked();

                    let trigger_from_keys = ctx.input(|i| {
                        let f5 = i.key_pressed(egui::Key::F5);
                        let cmd_r = i.modifiers.command && i.key_pressed(egui::Key::R);
                        f5 || cmd_r
                    });

                    if clicked || trigger_from_keys {
                        task_manager.request_refresh();
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Self::show_status_indicators(
                        ui,
                        websocket_manager,
                        mpv_connected,
                        anki_connected,
                    );
                });
            });
        });
    }

    fn show_status_indicators(
        ui: &mut egui::Ui,
        websocket_manager: &WebSocketManager,
        mpv_connected: bool,
        anki_connected: bool,
    ) {
        let server_state = websocket_manager.get_server_state();
        let asbplayer_connected = websocket_manager.has_clients();

        let (asbplayer_color, asbplayer_tooltip) = match server_state {
            ServerState::Running if asbplayer_connected => {
                (egui::Color32::from_rgb(0, 200, 0), "Connected to asbplayer".to_string())
            }
            ServerState::Running => (
                egui::Color32::from_rgb(200, 200, 0),
                "WebSocket server running - waiting for asbplayer".to_string(),
            ),
            ServerState::Error(ref err) => {
                (egui::Color32::from_rgb(200, 0, 0), format!("WebSocket server error: {}", err))
            }
            ServerState::Starting => {
                (egui::Color32::from_rgb(100, 100, 200), "WebSocket server starting...".to_string())
            }
            ServerState::Stopped => {
                (egui::Color32::from_rgb(100, 100, 100), "WebSocket server stopped".to_string())
            }
        };

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.small("asbplayer").on_hover_text(&asbplayer_tooltip);
            ui.small(egui::RichText::new("●").color(asbplayer_color))
                .on_hover_text(&asbplayer_tooltip);
        });

        ui.add_space(3.0);

        // MPV indicator
        let (mpv_color, mpv_tooltip) = if mpv_connected {
            (egui::Color32::from_rgb(0, 200, 0), "MPV detected - using MPV mode")
        } else {
            (egui::Color32::from_rgb(100, 100, 100), "MPV not detected")
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.small("mpv").on_hover_text(mpv_tooltip);
            ui.small(egui::RichText::new("●").color(mpv_color)).on_hover_text(mpv_tooltip);
        });

        ui.add_space(3.0);

        let anki_color = if anki_connected {
            egui::Color32::from_rgb(0, 200, 0)
        } else {
            egui::Color32::from_rgb(200, 80, 80)
        };

        let anki_tooltip =
            if anki_connected { "Connected to Anki" } else { "Not Connected to Anki" };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.small("Anki").on_hover_text(anki_tooltip);
            ui.small(egui::RichText::new("●").color(anki_color)).on_hover_text(anki_tooltip);
        });
    }
}

fn open_folder(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path does not exist: {}", path.display()),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn()?;
    }

    Ok(())
}
