use eframe::egui;
use egui::{Color32, Sense, Stroke, Vec2};

use super::{ControllerApp, ProfileAction, RenameState};
use crate::preview::{draw_controller, draw_controller_map};
use crate::settings::*;
use crate::theme::*;
use crate::widgets::*;

impl ControllerApp {
    pub(super) fn draw_overview(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        self.device_card(ui);
        ui.add_space(16.0);
        ui.columns(3, |columns| {
            metric_card(
                &mut columns[0],
                "ACTIVE PROFILE",
                self.profile_name(),
                "Ready to edit",
                BLUE,
            );
            metric_card(
                &mut columns[1],
                "BUTTONS MAPPED",
                &format!("{} / 18", self.bindings.len()),
                "Customizable controls",
                GREEN,
            );
            metric_card(
                &mut columns[2],
                "INPUT STATUS",
                if self.selected_gamepad.is_some() {
                    "Connected"
                } else {
                    "Waiting"
                },
                if self.virtual_gamepad.is_some() {
                    "Xbox output is active"
                } else if self.selected_gamepad.is_some() {
                    "Xbox output is off"
                } else {
                    "Connect a controller"
                },
                Color32::from_rgb(235, 153, 66),
            );
        });
        ui.add_space(18.0);
        ui.columns(2, |columns| {
            card(&mut columns[0], |ui| {
                ui.horizontal(|ui| {
                    section_heading(ui, "Quick setup", "Your current layout at a glance");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.link("Edit map  →").clicked() {
                            self.page = "Button mapping".into();
                        }
                    });
                });
                ui.add_space(11.0);
                for (action, binding) in ["Move up", "Confirm", "Cancel", "Pause"]
                    .into_iter()
                    .zip([0, 4, 5, 10])
                {
                    binding_row(
                        ui,
                        action,
                        self.bindings
                            .get(binding)
                            .map(String::as_str)
                            .unwrap_or("Not set"),
                        None,
                    );
                }
            });
            card(&mut columns[1], |ui| {
                ui.horizontal(|ui| {
                    section_heading(ui, "Controller preview", "A visual guide to your layout");
                });
                ui.add_space(3.0);
                draw_controller(ui, &self.pad_view());
            });
        });
        self.notice_line(ui);
    }

    pub(super) fn device_card(&mut self, ui: &mut egui::Ui) {
        let on = self.virtual_gamepad.is_some();
        let connected = self.selected_gamepad.is_some();
        let (title, detail) = if on {
            (
                "Xbox output is on",
                "Games now see an Xbox 360 controller. Keep DualLink open while you play.",
            )
        } else if connected {
            (
                "Controller ready",
                "Turn on Xbox output so games that expect an Xbox controller can use it.",
            )
        } else {
            (
                "Connect your PS4 controller",
                "Plug it in with USB or pair it over Bluetooth. Xbox output can turn on by itself.",
            )
        };
        let mut toggle = false;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
                ui.painter().circle_filled(
                    rect.center(),
                    26.0,
                    if on {
                        Color32::from_rgb(222, 245, 235)
                    } else {
                        Color32::from_rgb(237, 241, 255)
                    },
                );
                if on {
                    let c = rect.center();
                    ui.painter().add(egui::Shape::line(
                        vec![
                            c + Vec2::new(-9.0, 0.5),
                            c + Vec2::new(-3.0, 7.0),
                            c + Vec2::new(9.5, -7.0),
                        ],
                        Stroke::new(3.0_f32, GREEN),
                    ));
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "⌁",
                        egui::FontId::proportional(27.0),
                        BLUE,
                    );
                }
                ui.add_space(5.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).size(17.0).strong().color(INK));
                    ui.label(egui::RichText::new(detail).size(12.0).color(MUTED));
                    if let (Some(error), false) = (&self.output_error, on) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            ui.label(
                                egui::RichText::new("Could not start Xbox output.")
                                    .size(11.5)
                                    .color(RED),
                            );
                            ui.hyperlink_to(
                                egui::RichText::new("Install the ViGEmBus driver").size(11.5),
                                DRIVER_URL,
                            );
                        });
                        ui.label(egui::RichText::new(error).size(10.0).color(MUTED));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if on {
                        if ui.add(secondary_button("Turn off", 120.0)).clicked() {
                            toggle = true;
                        }
                    } else if ui
                        .add_enabled(connected, primary_button("Turn on Xbox output", 190.0))
                        .on_disabled_hover_text("Connect a controller first")
                        .clicked()
                    {
                        toggle = true;
                    }
                });
            });
        });
        if toggle {
            self.set_output(!on);
        }
    }

    pub(super) fn draw_mapping(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        card(ui, |ui| {
            section_heading(
                ui,
                "Controller map",
                "Each PS4 control and the Xbox control it sends. Lights up live as you press.",
            );
            ui.add_space(10.0);
            draw_controller_map(ui, &self.pad_view(), &self.bindings);
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            section_heading(ui, "Button mapping", "Choose an Xbox control for each PS4 button or trigger");
            ui.add_space(14.0);
            let actions = [
                "D-pad Up", "D-pad Down", "D-pad Left", "D-pad Right",
                "Cross (bottom face)", "Circle (right face)", "Square (left face)", "Triangle (top face)",
                "L1 (left bumper)", "R1 (right bumper)", "L2 (left trigger)", "R2 (right trigger)",
                "Options", "Share", "L3 (left stick click)", "R3 (right stick click)",
                "PS button", "Touchpad click",
            ];
            ui.columns(2, |columns| {
                for (column_index, column) in columns.iter_mut().enumerate() {
                    let range = if column_index == 0 { 0..9 } else { 9..18 };
                    for index in range {
                        mapping_row(column, index, actions[index], self.bindings.get_mut(index));
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    "These profile mappings are sent to the virtual Xbox controller when output is enabled. DualShock 4 touchpad clicks are read separately from the PS button using the controller USB HID report.",
                )
                .size(11.0)
                .color(MUTED),
            );
        });
        self.notice_line(ui);
    }

    pub(super) fn draw_input_test(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.columns(2, |columns| {
            card(&mut columns[0], |ui| {
                section_heading(
                    ui,
                    "Stick positions",
                    if self.selected_gamepad.is_some() {
                        "Live readings from your connected controller"
                    } else {
                        "Preview stick movement with the sliders"
                    },
                );
                ui.add_space(18.0);
                ui.add_enabled_ui(self.selected_gamepad.is_none(), |ui| {
                    stick_controls(ui, "Left stick", &mut self.left_x, &mut self.left_y);
                    ui.add_space(18.0);
                    stick_controls(ui, "Right stick", &mut self.right_x, &mut self.right_y);
                });
            });
            card(&mut columns[1], |ui| {
                section_heading(
                    ui,
                    "Live preview",
                    if self.selected_gamepad.is_some() {
                        "Reading the connected controller"
                    } else {
                        "Illustrative controls only"
                    },
                );
                ui.add_space(8.0);
                draw_controller(ui, &self.pad_view());
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Triggers")
                        .size(13.0)
                        .strong()
                        .color(INK),
                );
                ui.add_space(5.0);
                ui.add_enabled_ui(self.selected_gamepad.is_none(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("L").strong().color(MUTED));
                        ui.add(
                            egui::Slider::new(&mut self.trigger_left, 0.0..=1.0).show_value(false),
                        );
                        ui.label(egui::RichText::new("R").strong().color(MUTED));
                        ui.add(
                            egui::Slider::new(&mut self.trigger_right, 0.0..=1.0).show_value(false),
                        );
                    });
                });
                ui.add_space(8.0);
                pill(
                    ui,
                    if self.selected_gamepad.is_some() {
                        "LIVE CONTROLLER INPUT"
                    } else {
                        "NO HARDWARE INPUT"
                    },
                    Color32::from_rgb(255, 246, 231),
                    Color32::from_rgb(177, 115, 32),
                );
            });
        });
        self.notice_line(ui);
    }

    pub(super) fn draw_profiles(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.columns(2, |columns| {
            card(&mut columns[0], |ui| {
                ui.horizontal(|ui| {
                    section_heading(ui, "Your profiles", "Choose, rename or remove layouts");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(secondary_button("+ New profile", 120.0)).clicked() {
                            self.add_profile();
                        }
                    });
                });
                ui.add_space(14.0);
                let mut action: Option<ProfileAction> = None;
                let can_delete = self.profiles.len() > 1;
                for index in 0..self.profiles.len() {
                    let name = self.profiles[index].name.clone();
                    let selected = self.selected_profile == index;
                    let editing = self
                        .renaming
                        .as_ref()
                        .is_some_and(|state| state.index == index);
                    let confirming = self.confirm_delete == Some(index);
                    let frame = egui::Frame::new()
                        .fill(if selected {
                            Color32::from_rgb(240, 244, 255)
                        } else {
                            Color32::WHITE
                        })
                        .stroke(Stroke::new(
                            1.0_f32,
                            if selected {
                                Color32::from_rgb(207, 217, 255)
                            } else {
                                LINE
                            },
                        ))
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(egui::Margin::same(12));
                    frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                if editing {
                                    if let Some(state) = self.renaming.as_mut() {
                                        let response = ui.add(
                                            egui::TextEdit::singleline(&mut state.text)
                                                .desired_width(190.0)
                                                .char_limit(32),
                                        );
                                        if state.focus {
                                            response.request_focus();
                                            state.focus = false;
                                        }
                                        let enter =
                                            ui.input(|input| input.key_pressed(egui::Key::Enter));
                                        let escape =
                                            ui.input(|input| input.key_pressed(egui::Key::Escape));
                                        if response.lost_focus() && enter {
                                            action = Some(ProfileAction::CommitRename);
                                        } else if escape {
                                            action = Some(ProfileAction::CancelRename);
                                        }
                                    }
                                    ui.label(
                                        egui::RichText::new("Enter to save, Esc to cancel")
                                            .size(11.0)
                                            .color(MUTED),
                                    );
                                } else {
                                    let title = ui
                                        .add(
                                            egui::Label::new(
                                                egui::RichText::new(&name)
                                                    .size(14.0)
                                                    .strong()
                                                    .color(INK),
                                            )
                                            .sense(Sense::click()),
                                        )
                                        .on_hover_text("Double-click to rename");
                                    if title.double_clicked() {
                                        action = Some(ProfileAction::StartRename(index));
                                    }
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} button bindings",
                                            self.profiles[index].bindings.len()
                                        ))
                                        .size(11.0)
                                        .color(MUTED),
                                    );
                                }
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if editing {
                                        if ui.button("Cancel").clicked() {
                                            action = Some(ProfileAction::CancelRename);
                                        }
                                        if ui.add(primary_button("Save", 70.0)).clicked() {
                                            action = Some(ProfileAction::CommitRename);
                                        }
                                    } else if confirming {
                                        let delete = egui::Button::new(
                                            egui::RichText::new("Delete")
                                                .strong()
                                                .color(Color32::WHITE),
                                        )
                                        .fill(RED);
                                        if ui.add(delete).clicked() {
                                            action = Some(ProfileAction::ConfirmDelete(index));
                                        }
                                        if ui.button("Keep").clicked() {
                                            action = Some(ProfileAction::CancelDelete);
                                        }
                                        ui.label(
                                            egui::RichText::new("Delete this profile?")
                                                .size(11.5)
                                                .color(RED),
                                        );
                                    } else {
                                        if ui
                                            .add_enabled(can_delete, egui::Button::new("Delete"))
                                            .on_disabled_hover_text("Keep at least one profile")
                                            .clicked()
                                        {
                                            action = Some(ProfileAction::AskDelete(index));
                                        }
                                        if ui.button("Rename").clicked() {
                                            action = Some(ProfileAction::StartRename(index));
                                        }
                                        if selected {
                                            pill(
                                                ui,
                                                "CURRENT",
                                                Color32::from_rgb(226, 234, 255),
                                                BLUE,
                                            );
                                        } else if ui.button("Use profile").clicked() {
                                            action = Some(ProfileAction::Use(index));
                                        }
                                    }
                                },
                            );
                        });
                    });
                    ui.add_space(8.0);
                }
                match action {
                    Some(ProfileAction::Use(index)) => self.set_profile(index),
                    Some(ProfileAction::StartRename(index)) => {
                        self.confirm_delete = None;
                        if let Some(profile) = self.profiles.get(index) {
                            self.renaming = Some(RenameState {
                                index,
                                text: profile.name.clone(),
                                focus: true,
                            });
                        }
                    }
                    Some(ProfileAction::CommitRename) => self.commit_rename(),
                    Some(ProfileAction::CancelRename) => self.renaming = None,
                    Some(ProfileAction::AskDelete(index)) => {
                        self.renaming = None;
                        self.confirm_delete = Some(index);
                    }
                    Some(ProfileAction::ConfirmDelete(index)) => self.delete_profile(index),
                    Some(ProfileAction::CancelDelete) => self.confirm_delete = None,
                    None => {}
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Renaming and deleting save right away. Use Save profile to keep button changes.",
                    )
                    .size(11.0)
                    .color(MUTED),
                );
            });
            card(&mut columns[1], |ui| {
                section_heading(ui, "Selected layout", "Edit bindings in Button mapping");
                ui.add_space(14.0);
                let selected_index = self.selected_profile;
                let selected_name = self.profile_name().to_owned();
                let mut rename = false;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("PROFILE NAME")
                                .size(9.5)
                                .strong()
                                .color(MUTED),
                        );
                        ui.label(
                            egui::RichText::new(&selected_name)
                                .size(19.0)
                                .strong()
                                .color(INK),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Rename").clicked() {
                            rename = true;
                        }
                    });
                });
                if rename {
                    self.confirm_delete = None;
                    self.renaming = Some(RenameState {
                        index: selected_index,
                        text: selected_name,
                        focus: true,
                    });
                }
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new(format!("{} controls configured", self.bindings.len()))
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add_space(16.0);
                for (action, index) in
                    [("Move up", 0), ("Confirm", 4), ("Cancel", 5), ("Pause", 10)]
                {
                    let binding = self
                        .bindings
                        .get(index)
                        .map(String::as_str)
                        .unwrap_or("Not set");
                    binding_row(ui, action, binding, None);
                }
                ui.add_space(12.0);
                if ui.button("Open button mapping  →").clicked() {
                    self.page = "Button mapping".into();
                }
            });
        });
        self.notice_line(ui);
    }

    pub(super) fn draw_help(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.columns(2, |columns| {
            card(&mut columns[0], |ui| {
                section_heading(ui, "Start playing", "Follow these steps each time you want to use your controller");
                ui.add_space(15.0);
                for (number, title, detail) in [
                    ("1", "Install the Xbox-output driver once", "Open Compatibility and use the official ViGEmBus link. Install it yourself and restart Windows if asked."),
                    ("2", "Connect your PS4 controller", "Plug it in with USB or pair it with Bluetooth. Keep it connected while you play."),
                    ("3", "Turn on Xbox output", "In Compatibility, choose your controller and press Enable Xbox output for games."),
                    ("4", "Start your game", "Leave this app open while playing. Choose Xbox 360 or XInput controller in the game's settings if it asks."),
                ] {
                    ui.horizontal_top(|ui| {
                        pill(ui, number, Color32::from_rgb(237, 241, 255), BLUE);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(title).size(12.0).strong().color(INK));
                            ui.label(egui::RichText::new(detail).size(11.0).color(MUTED));
                        });
                    });
                    ui.add_space(13.0);
                }
                if ui.button("Open controller setup").clicked() {
                    self.page = "Compatibility".into();
                }
            });
            card(&mut columns[1], |ui| {
                section_heading(ui, "Change controls", "Save a layout that feels right for each game");
                ui.add_space(13.0);
                for (title, detail) in [
                    ("Button mapping", "Choose the Xbox action for each PS4 control from its menu."),
                    ("Profiles", "Make a separate layout for another game. Save profile stores your layouts on this computer."),
                    ("Input test", "Connect your controller to see live stick and trigger movement."),
                ] {
                    ui.label(egui::RichText::new(title).size(12.0).strong().color(INK));
                    ui.label(egui::RichText::new(detail).size(11.0).color(MUTED));
                    ui.add_space(10.0);
                }
                ui.separator();
                ui.add_space(10.0);
                section_heading(ui, "If a game doesn't respond", "A few quick things to check");
                ui.add_space(8.0);
                ui.label(egui::RichText::new("• Make sure Xbox output says active and leave this app open.\n• Keep the PS4 controller connected.\n• Some very old games only support DirectInput and may not recognize Xbox output.\n• Games vary; protected online modes can block virtual controllers. Follow the game's rules.").size(11.0).color(INK));
                ui.add_space(10.0);
                ui.hyperlink_to("Open driver setup", "https://github.com/nefarius/ViGEmBus/releases");
            });
        });
    }

    pub(super) fn draw_compatibility(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.columns(2, |columns| {
            card(&mut columns[0], |ui| {
                section_heading(ui, "Use a PS4 controller with PC games", "Create an Xbox 360-style controller that many older games understand");
                ui.add_space(14.0);
                for (number, instruction) in [
                    "Connect the DualShock 4 by USB or Bluetooth.",
                    "Xbox output turns on by itself, or use the switch at the top.",
                    "Leave DualLink open, then start your game.",
                ].iter().enumerate() {
                    ui.horizontal(|ui| {
                        pill(ui, &format!("{}", number + 1), Color32::from_rgb(237, 241, 255), BLUE);
                        ui.label(egui::RichText::new(*instruction).size(12.0).color(INK));
                    });
                    ui.add_space(7.0);
                }
                ui.add_space(8.0);
                if let Some(error) = &self.input_error {
                    ui.colored_label(Color32::from_rgb(177, 80, 65), error);
                }
                let selected_name = self.selected_gamepad
                    .and_then(|selected| self.connected_gamepads.iter().find(|(id, _)| *id == selected).map(|(_, name)| name.as_str()))
                    .unwrap_or("No controller detected");
                egui::ComboBox::from_label("Controller")
                    .selected_text(selected_name)
                    .show_ui(ui, |ui| {
                        for (id, name) in &self.connected_gamepads {
                            ui.selectable_value(&mut self.selected_gamepad, Some(*id), name);
                        }
                    });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if toggle_switch(ui, self.auto_enable).clicked() {
                        self.auto_enable = !self.auto_enable;
                        if let Err(error) = save_settings(&Settings { auto_enable: self.auto_enable }) {
                            self.notice = format!("Could not save settings: {error}");
                        }
                    }
                    ui.label(egui::RichText::new("Turn on Xbox output automatically when a controller connects").size(12.0).color(INK));
                });
                ui.add_space(10.0);
                if self.virtual_gamepad.is_some() {
                    if ui.add(secondary_button("Turn off Xbox output", 190.0)).clicked() {
                        self.set_output(false);
                    }
                } else if ui.add_enabled(self.selected_gamepad.is_some(), primary_button("Turn on Xbox output", 190.0)).clicked() {
                    self.set_output(true);
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("●").color(if self.virtual_gamepad.is_some() { GREEN } else { MUTED }));
                    ui.label(egui::RichText::new(if self.virtual_gamepad.is_some() { "Virtual Xbox controller is active" } else { "Virtual Xbox controller is off" }).size(12.0).strong().color(INK));
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new(&self.gamepad_status).size(11.0).color(MUTED));
            });
            card(&mut columns[1], |ui| {
                section_heading(ui, "One-time setup", "Xbox output needs the signed ViGEmBus driver");
                ui.add_space(13.0);
                ui.label(egui::RichText::new("This app never installs drivers or changes game files. Install ViGEmBus from its official project, restart if requested, reopen this app, enable Xbox output, and start the game while this app stays open.").size(12.0).color(INK));
                ui.add_space(13.0);
                ui.hyperlink_to("Open the official ViGEmBus project", "https://github.com/nefarius/ViGEmBus/releases");
                ui.add_space(18.0);
                section_heading(ui, "Standard button translation", "Your saved mapping profile selects the Xbox buttons");
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Sticks and analog triggers pass through; face buttons, D-pad, bumpers, menu buttons, stick clicks, the PS button, and the separately detected touchpad click use the active profile.").size(11.5).color(MUTED));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("This creates an Xbox 360-style XInput controller. Some very old DirectInput-only games may not accept it; check the game's controller settings. If the game sees both controllers, adjust its controller settings; this app needs the PS4 controller to stay connected.").size(11.0).color(MUTED));
                ui.add_space(11.0);
                if self.virtual_gamepad.is_some() {
                    pill(ui, "XINPUT OUTPUT ACTIVE · SUPPORT VARIES BY GAME", Color32::from_rgb(231, 247, 239), GREEN);
                } else {
                    pill(ui, "OUTPUT NOT ENABLED", Color32::from_rgb(255, 246, 231), Color32::from_rgb(177, 115, 32));
                }
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Closing this app removes its virtual controller. Online games may block third-party virtual controllers; follow each game's rules.").size(10.5).color(MUTED));
            });
        });
        self.notice_line(ui);
    }
}
