use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use gilrs::{Axis, Button as PadButton, EventType, GamepadId, Gilrs};
use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::Duration;
use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};

const BG: Color32 = Color32::from_rgb(243, 246, 250);
const NAVY: Color32 = Color32::from_rgb(21, 33, 54);
const BLUE: Color32 = Color32::from_rgb(77, 111, 240);
const INK: Color32 = Color32::from_rgb(35, 48, 69);
const MUTED: Color32 = Color32::from_rgb(124, 137, 157);
const GREEN: Color32 = Color32::from_rgb(40, 166, 119);
const LINE: Color32 = Color32::from_rgb(231, 235, 241);
const RED: Color32 = Color32::from_rgb(214, 69, 69);
const SIDEBAR_MUTED: Color32 = Color32::from_rgb(143, 162, 193);
const DRIVER_URL: &str = "https://github.com/nefarius/ViGEmBus/releases";

/// In-progress rename of one profile on the Profiles page.
struct RenameState {
    index: usize,
    text: String,
    focus: bool,
}

#[derive(Clone, Copy)]
enum ProfileAction {
    Use(usize),
    StartRename(usize),
    CommitRename,
    CancelRename,
    AskDelete(usize),
    ConfirmDelete(usize),
    CancelDelete,
}

#[derive(Serialize, Deserialize)]
struct Settings {
    /// Turn Xbox output on by itself whenever a controller is connected.
    #[serde(default = "default_true")]
    auto_enable: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self { auto_enable: true }
    }
}

fn settings_path() -> Result<PathBuf, String> {
    profiles_path().map(|path| path.with_file_name("settings.json"))
}

fn load_settings() -> Settings {
    settings_path()
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(path, json).map_err(|error| error.to_string())
}

/// Whether Xbox output should start by itself right now.
fn should_auto_enable(
    auto_enable: bool,
    controller_connected: bool,
    already_attempted: bool,
    turned_off_by_user: bool,
    output_active: bool,
) -> bool {
    auto_enable && controller_connected && !already_attempted && !turned_off_by_user && !output_active
}

fn unique_profile_name(profiles: &[Profile]) -> String {
    let mut number = profiles.len() + 1;
    loop {
        let candidate = format!("My profile {number}");
        let lowered = candidate.to_lowercase();
        if !profiles.iter().any(|profile| profile.name.to_lowercase() == lowered) {
            return candidate;
        }
        number += 1;
    }
}

/// Which profile should be selected after `deleted` is removed (`new_len` is the length afterwards).
fn selection_after_delete(selected: usize, deleted: usize, new_len: usize) -> usize {
    if deleted < selected {
        selected - 1
    } else if deleted == selected {
        deleted.min(new_len.saturating_sub(1))
    } else {
        selected
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Profile {
    name: String,
    bindings: Vec<String>,
}

struct ControllerApp {
    page: String,
    profiles: Vec<Profile>,
    selected_profile: usize,
    bindings: Vec<String>,
    left_x: f32,
    left_y: f32,
    right_x: f32,
    right_y: f32,
    trigger_left: f32,
    trigger_right: f32,
    pressed: [bool; 18],
    touchpad_pressed: Arc<AtomicBool>,
    auto_enable: bool,
    auto_attempted: bool,
    user_turned_off: bool,
    output_error: Option<String>,
    renaming: Option<RenameState>,
    confirm_delete: Option<usize>,
    notice: String,
    gilrs: Option<Gilrs>,
    input_error: Option<String>,
    connected_gamepads: Vec<(GamepadId, String)>,
    selected_gamepad: Option<GamepadId>,
    virtual_gamepad: Option<Xbox360Wired<Client>>,
    gamepad_status: String,
    screenshot_path: Option<PathBuf>,
    screenshot_requested: bool,
    screenshot_frames: u32,
}

impl ControllerApp {
    fn new(screenshot_path: Option<PathBuf>) -> Self {
        let defaults = vec![
            Profile {
                name: "Everyday".into(),
                bindings: default_bindings(),
            },
            Profile {
                name: "Arcade".into(),
                bindings: default_bindings(),
            },
        ];
        let (mut profiles, notice) = match load_profiles() {
            Ok(Some(profiles)) if !profiles.is_empty() => (profiles, "Profiles loaded".to_owned()),
            Ok(_) => (defaults, "".to_owned()),
            Err(error) => {
                eprintln!("Could not load controller profiles: {error}");
                (
                    defaults,
                    "Saved profiles could not be loaded; using defaults".to_owned(),
                )
            }
        };
        for profile in &mut profiles {
            profile.bindings = migrate_profile_bindings(std::mem::take(&mut profile.bindings));
        }
        let bindings = profiles[0].bindings.clone();
        let touchpad_pressed = start_ds4_touchpad_reader();
        let (gilrs, input_error) = match Gilrs::new() {
            Ok(gilrs) => (Some(gilrs), None),
            Err(error) => (
                None,
                Some(format!("Controller input could not start: {error}")),
            ),
        };
        Self {
            page: "Overview".into(),
            profiles,
            selected_profile: 0,
            bindings,
            left_x: 0.0,
            left_y: 0.0,
            right_x: 0.0,
            right_y: 0.0,
            trigger_left: 0.0,
            trigger_right: 0.0,
            pressed: [false; 18],
            touchpad_pressed,
            auto_enable: load_settings().auto_enable,
            auto_attempted: false,
            user_turned_off: false,
            output_error: None,
            renaming: None,
            confirm_delete: None,
            notice,
            gilrs,
            input_error,
            connected_gamepads: Vec::new(),
            selected_gamepad: None,
            virtual_gamepad: None,
            gamepad_status: "Connect a DualShock 4 controller to begin".into(),
            screenshot_path,
            screenshot_requested: false,
            screenshot_frames: 0,
        }
    }

    fn set_profile(&mut self, index: usize) {
        if let Some(profile) = self.profiles.get(index) {
            self.selected_profile = index;
            self.renaming = None;
            self.confirm_delete = None;
            self.bindings = profile.bindings.clone();
            self.notice = format!("{} profile selected", profile.name);
        }
    }

    fn write_profiles(&self) -> Result<(), String> {
        let path = profiles_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let json = serde_json::to_vec_pretty(&self.profiles).map_err(|error| error.to_string())?;
        std::fs::write(path, json).map_err(|error| error.to_string())
    }

    fn save_profiles(&mut self) {
        if let Some(profile) = self.profiles.get_mut(self.selected_profile) {
            profile.bindings = self.bindings.clone();
        }
        match self.write_profiles() {
            Ok(()) => self.notice = "Profiles saved on this computer".into(),
            Err(error) => {
                eprintln!("Could not save controller profiles: {error}");
                self.notice = format!("Could not save profiles: {error}");
            }
        }
    }

    /// Saves the list of profiles right away after a rename, delete or new profile.
    /// Unsaved edits to the current layout are not written; use Save profile for those.
    fn persist_profiles(&mut self, message: String) {
        match self.write_profiles() {
            Ok(()) => self.notice = message,
            Err(error) => {
                eprintln!("Could not save controller profiles: {error}");
                self.notice = format!("{message}, but it could not be saved: {error}");
            }
        }
    }

    fn add_profile(&mut self) {
        let name = unique_profile_name(&self.profiles);
        self.profiles.push(Profile {
            name: name.clone(),
            bindings: self.bindings.clone(),
        });
        let index = self.profiles.len() - 1;
        self.set_profile(index);
        self.renaming = Some(RenameState {
            index,
            text: name.clone(),
            focus: true,
        });
        self.persist_profiles(format!("Created \"{name}\" - type a name and press Enter"));
    }

    fn commit_rename(&mut self) {
        let Some(state) = self.renaming.as_ref() else {
            return;
        };
        let index = state.index;
        let name = state.text.trim().to_owned();
        if name.is_empty() {
            self.notice = "A profile name cannot be empty".into();
            return;
        }
        let lowered = name.to_lowercase();
        if self
            .profiles
            .iter()
            .enumerate()
            .any(|(other, profile)| other != index && profile.name.to_lowercase() == lowered)
        {
            self.notice = format!("A profile named \"{name}\" already exists");
            return;
        }
        if let Some(profile) = self.profiles.get_mut(index) {
            profile.name = name.clone();
        }
        self.renaming = None;
        self.persist_profiles(format!("Renamed profile to \"{name}\""));
    }

    fn delete_profile(&mut self, index: usize) {
        self.confirm_delete = None;
        if self.profiles.len() <= 1 || index >= self.profiles.len() {
            self.notice = "Keep at least one profile".into();
            return;
        }
        let removed = self.profiles.remove(index);
        self.renaming = None;
        let next = selection_after_delete(self.selected_profile, index, self.profiles.len());
        if index == self.selected_profile {
            self.set_profile(next);
        } else {
            self.selected_profile = next;
        }
        self.persist_profiles(format!("Deleted profile \"{}\"", removed.name));
    }

    fn draw_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(236.0)
            .frame(
                egui::Frame::new()
                    .fill(NAVY)
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
                        ui.painter()
                            .rect_filled(rect, egui::CornerRadius::same(12), BLUE);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "D",
                            egui::FontId::proportional(20.0),
                            Color32::WHITE,
                        );
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.add_space(1.0);
                            ui.label(
                                egui::RichText::new("DualLink")
                                    .size(18.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new("CONTROLLER STUDIO")
                                    .size(8.5)
                                    .strong()
                                    .color(SIDEBAR_MUTED),
                            );
                        });
                    });
                    ui.add_space(32.0);
                    ui.label(
                        egui::RichText::new("WORKSPACE")
                            .size(10.0)
                            .strong()
                            .color(SIDEBAR_MUTED),
                    );
                    ui.add_space(8.0);
                    for (label, marker) in [
                        ("Overview", "◈"),
                        ("Button mapping", "⌘"),
                        ("Input test", "◎"),
                        ("Profiles", "▤"),
                        ("Compatibility", "⇄"),
                        ("How to use", "?"),
                    ] {
                        let selected = self.page == label;
                        if nav_item(ui, marker, label, selected).clicked() {
                            self.page = label.into();
                        }
                        ui.add_space(2.0);
                    }
                    ui.add_space(22.0);
                    ui.label(
                        egui::RichText::new("YOUR DEVICE")
                            .size(10.0)
                            .strong()
                            .color(SIDEBAR_MUTED),
                    );
                    ui.add_space(8.0);
                    let connected = self.selected_gamepad.is_some();
                    let output_on = self.virtual_gamepad.is_some();
                    let name = self
                        .selected_gamepad
                        .and_then(|selected| {
                            self.connected_gamepads
                                .iter()
                                .find(|(id, _)| *id == selected)
                                .map(|(_, name)| name.as_str())
                        })
                        .unwrap_or("No controller");
                    egui::Frame::new()
                        .fill(Color32::from_rgb(30, 44, 70))
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("●")
                                        .size(11.0)
                                        .color(if connected { GREEN } else { SIDEBAR_MUTED }),
                                );
                                ui.label(
                                    egui::RichText::new(name)
                                        .size(12.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                );
                            });
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(if output_on {
                                    "Xbox output is on"
                                } else {
                                    "Xbox output is off"
                                })
                                .size(10.5)
                                .color(if output_on {
                                    Color32::from_rgb(110, 214, 168)
                                } else {
                                    SIDEBAR_MUTED
                                }),
                            );
                        });
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("A calm space to tune your\ncontroller layout.")
                                .size(11.0)
                                .color(SIDEBAR_MUTED),
                        );
                        ui.label(
                            egui::RichText::new("DUALLINK  ·  PERSONAL")
                                .size(9.0)
                                .strong()
                                .color(SIDEBAR_MUTED),
                        );
                    });
                });
            });
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(page_title(&self.page))
                        .size(25.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    egui::RichText::new(page_subtitle(&self.page))
                        .size(12.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(primary_button("Save profile", 120.0)).clicked() {
                    self.save_profiles();
                }
                ui.add_space(12.0);
                self.output_switch(ui);
            });
        });
    }

    /// The always-visible Xbox output switch shown at the top of every page.
    fn output_switch(&mut self, ui: &mut egui::Ui) {
        let on = self.virtual_gamepad.is_some();
        let connected = self.selected_gamepad.is_some();
        let failed = self.output_error.is_some() && !on;
        let (dot, status) = if on {
            (GREEN, "On - games see an Xbox controller")
        } else if failed {
            (RED, "Could not start - driver needed")
        } else if connected {
            (MUTED, "Off - switch on to use in games")
        } else {
            (MUTED, "Connect your controller")
        };
        let mut toggle = false;
        egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(
                1.0_f32,
                if on { Color32::from_rgb(168, 224, 199) } else { LINE },
            ))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin {
                left: 14,
                right: 12,
                top: 8,
                bottom: 8,
            })
            .shadow(soft_shadow())
            .show(ui, |ui| {
                ui.set_width(300.0);
                // Explicit direction: `horizontal` would flip inside the header's right-to-left row.
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Xbox output")
                                .size(12.5)
                                .strong()
                                .color(INK),
                        );
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            ui.label(egui::RichText::new("●").size(9.0).color(dot));
                            ui.label(egui::RichText::new(status).size(10.5).color(MUTED));
                            if failed {
                                ui.hyperlink_to(
                                    egui::RichText::new("Get ViGEmBus").size(10.5),
                                    DRIVER_URL,
                                );
                            }
                        });
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let usable = on || connected;
                        if ui
                            .add_enabled_ui(usable, |ui| toggle_switch(ui, on))
                            .inner
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

    fn draw_overview(&mut self, ui: &mut egui::Ui) {
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

    fn device_card(&mut self, ui: &mut egui::Ui) {
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

    fn draw_mapping(&mut self, ui: &mut egui::Ui) {
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

    fn draw_input_test(&mut self, ui: &mut egui::Ui) {
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

    fn draw_profiles(&mut self, ui: &mut egui::Ui) {
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

    fn poll_controller(&mut self) {
        let touchpad_pressed = self.touchpad_pressed.load(Ordering::Relaxed);
        let Some(gilrs) = self.gilrs.as_mut() else {
            return;
        };
        while let Some(event) = gilrs.next_event() {
            if matches!(event.event, EventType::Disconnected)
                && self.selected_gamepad == Some(event.id)
            {
                self.selected_gamepad = None;
            }
        }
        self.connected_gamepads = gilrs
            .gamepads()
            .filter_map(|(id, gamepad)| {
                gamepad
                    .is_connected()
                    .then(|| (id, gamepad.name().to_owned()))
            })
            .collect();
        if !self
            .connected_gamepads
            .iter()
            .any(|(id, _)| Some(*id) == self.selected_gamepad)
        {
            self.selected_gamepad = self.connected_gamepads.first().map(|(id, _)| *id);
        }

        let state = self.selected_gamepad.map(|id| {
            let gamepad = gilrs.gamepad(id);
            let left_x = gamepad.value(Axis::LeftStickX);
            let left_y = gamepad.value(Axis::LeftStickY);
            let right_x = gamepad.value(Axis::RightStickX);
            let right_y = gamepad.value(Axis::RightStickY);
            let trigger_left = read_trigger(&gamepad, PadButton::LeftTrigger2, Axis::LeftZ);
            let trigger_right = read_trigger(&gamepad, PadButton::RightTrigger2, Axis::RightZ);
            let sources = [
                PadButton::DPadUp,
                PadButton::DPadDown,
                PadButton::DPadLeft,
                PadButton::DPadRight,
                PadButton::South,
                PadButton::East,
                PadButton::West,
                PadButton::North,
                PadButton::LeftTrigger,
                PadButton::RightTrigger,
                PadButton::LeftTrigger2,
                PadButton::RightTrigger2,
                PadButton::Start,
                PadButton::Select,
                PadButton::LeftThumb,
                PadButton::RightThumb,
                PadButton::Mode, // The PS / Guide button, not the touchpad click.
            ];
            let mut pressed = [false; 18];
            for (index, source) in sources.into_iter().enumerate() {
                pressed[index] = gamepad.is_pressed(source);
            }
            // The touchpad click is not reliably exposed by gilrs on Windows.
            // It is read independently from the Sony DualShock 4 USB HID report.
            pressed[17] = touchpad_pressed;
            let mut state =
                translate_controls(pressed, trigger_left, trigger_right, &self.bindings);
            state.thumb_lx = (left_x * i16::MAX as f32) as i16;
            state.thumb_ly = (left_y * i16::MAX as f32) as i16;
            state.thumb_rx = (right_x * i16::MAX as f32) as i16;
            state.thumb_ry = (right_y * i16::MAX as f32) as i16;
            (
                gamepad.name().to_owned(),
                left_x,
                left_y,
                right_x,
                right_y,
                trigger_left,
                trigger_right,
                state,
                pressed,
            )
        });

        if let Some((name, left_x, left_y, right_x, right_y, trigger_left, trigger_right, state, pressed)) =
            state
        {
            self.gamepad_status = format!("Connected: {name}");
            self.left_x = left_x;
            self.left_y = left_y;
            self.right_x = right_x;
            self.right_y = right_y;
            self.trigger_left = trigger_left;
            self.trigger_right = trigger_right;
            self.pressed = pressed;
            if let Some(virtual_gamepad) = self.virtual_gamepad.as_mut() {
                if let Err(error) = virtual_gamepad.update(&state) {
                    self.notice = format!("Virtual controller update failed: {error}");
                    eprintln!("Virtual controller update failed: {error}");
                    self.virtual_gamepad = None;
                }
            }
        } else {
            self.gamepad_status = "No controller detected".into();
            self.left_x = 0.0;
            self.left_y = 0.0;
            self.right_x = 0.0;
            self.right_y = 0.0;
            self.trigger_left = 0.0;
            self.trigger_right = 0.0;
            self.pressed = [false; 18];
            if let Some(virtual_gamepad) = self.virtual_gamepad.as_mut() {
                if let Err(error) = virtual_gamepad.update(&XGamepad::default()) {
                    self.notice = format!("Virtual controller update failed: {error}");
                    eprintln!("Virtual controller update failed: {error}");
                    self.virtual_gamepad = None;
                }
            }
        }
    }

    /// Turns Xbox output on or off from any page (header switch, Overview, Compatibility).
    fn set_output(&mut self, on: bool) {
        if on {
            self.user_turned_off = false;
            self.enable_virtual_gamepad();
        } else {
            self.user_turned_off = true;
            self.disable_virtual_gamepad();
        }
    }

    /// Starts Xbox output by itself when a controller connects, if that setting is on.
    /// It tries once per connection, so a missing driver does not cause a retry every frame.
    fn maybe_auto_enable(&mut self) {
        if self.selected_gamepad.is_none() {
            self.auto_attempted = false;
            self.user_turned_off = false;
            return;
        }
        if should_auto_enable(
            self.auto_enable,
            true,
            self.auto_attempted,
            self.user_turned_off,
            self.virtual_gamepad.is_some(),
        ) {
            self.auto_attempted = true;
            self.enable_virtual_gamepad();
        }
    }

    fn enable_virtual_gamepad(&mut self) {
        if self.selected_gamepad.is_none() {
            self.notice = "Connect a controller before enabling game output".into();
            return;
        }
        let result = (|| {
            let client = Client::connect().map_err(|error| error.to_string())?;
            let mut target = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
            target.plugin().map_err(|error| error.to_string())?;
            target.wait_ready().map_err(|error| error.to_string())?;
            Ok::<_, String>(target)
        })();
        match result {
            Ok(target) => {
                self.virtual_gamepad = Some(target);
                self.output_error = None;
                self.notice = "Virtual Xbox controller is ready for games".into();
            }
            Err(error) => {
                self.notice = format!("Could not start Xbox output. Install the signed ViGEmBus driver, then try again. Details: {error}");
                eprintln!("Could not start virtual Xbox controller: {error}");
                self.output_error = Some(error);
            }
        }
    }

    fn disable_virtual_gamepad(&mut self) {
        if let Some(mut target) = self.virtual_gamepad.take() {
            if let Err(error) = target.unplug() {
                self.notice = format!("Virtual controller could not be unplugged cleanly: {error}");
                eprintln!("Virtual controller unplug failed: {error}");
                return;
            }
            self.notice = "Virtual Xbox controller disabled".into();
        }
    }

    fn draw_help(&mut self, ui: &mut egui::Ui) {
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

    fn draw_compatibility(&mut self, ui: &mut egui::Ui) {
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

    fn draw_ui(&mut self, ctx: &egui::Context) {
        self.draw_sidebar(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::same(26)),
            )
            .show(ctx, |ui| {
                self.draw_header(ui);
                ui.add_space(18.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page.as_str() {
                        "Button mapping" => self.draw_mapping(ui),
                        "Input test" => self.draw_input_test(ui),
                        "Profiles" => self.draw_profiles(ui),
                        "Compatibility" => self.draw_compatibility(ui),
                        "How to use" => self.draw_help(ui),
                        _ => self.draw_overview(ui),
                    });
            });
    }

    fn pad_view(&self) -> PadView {
        PadView {
            left_x: self.left_x,
            left_y: self.left_y,
            right_x: self.right_x,
            right_y: self.right_y,
            trigger_left: self.trigger_left,
            trigger_right: self.trigger_right,
            pressed: self.pressed,
        }
    }

    fn profile_name(&self) -> &str {
        self.profiles
            .get(self.selected_profile)
            .map(|profile| profile.name.as_str())
            .unwrap_or("Everyday")
    }

    fn notice_line(&self, ui: &mut egui::Ui) {
        if !self.notice.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(&self.notice).size(11.0).color(MUTED));
        }
    }
}

impl eframe::App for ControllerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_controller();
        self.maybe_auto_enable();
        ctx.request_repaint_after(Duration::from_millis(16));
        let captured = ctx.input(|input| {
            input.raw.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(captured) = captured {
            if let Some(path) = self.screenshot_path.as_ref() {
                let pixels: Vec<u8> = captured
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                let result = (|| {
                    if let Some(parent) = path
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                    {
                        std::fs::create_dir_all(parent)?;
                    }
                    image::save_buffer(
                        path,
                        &pixels,
                        captured.size[0] as u32,
                        captured.size[1] as u32,
                        image::ColorType::Rgba8,
                    )
                    .map_err(|error| std::io::Error::other(error.to_string()))
                })();
                if let Err(error) = result {
                    eprintln!("Could not save screenshot: {error}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        self.draw_ui(ctx);

        if self.screenshot_path.is_some() && !self.screenshot_requested {
            self.screenshot_frames += 1;
            if self.screenshot_frames >= 120 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.screenshot_requested = true;
                ctx.request_repaint();
            } else {
                ctx.request_repaint();
            }
        }
    }
}

/// Extracts the touchpad-click state from a DualShock 4 input report.
/// USB (and the simple Bluetooth mode) use report 0x01 with the three button bytes at 5..=7;
/// the full Bluetooth report 0x11 carries the same bytes two positions later (7..=9).
/// Returns None for anything else, such as an empty read or an unrelated report.
fn ds4_touchpad_click_from_report(report: &[u8]) -> Option<bool> {
    let button_byte = match report.first() {
        Some(0x01) if report.len() >= 8 => report[7],
        Some(0x11) if report.len() >= 10 => report[9],
        _ => return None,
    };
    Some(button_byte & 0x02 != 0)
}

/// Reads the DualShock 4 V2 touchpad-click bit directly from its USB HID input report.
///
/// Sony's DS4 USB report is 64 bytes (report ID 0x01). The three button bytes
/// begin at offsets 5..=7; bit 1 of the third button byte (offset 7) is the
/// touchpad click. The PS button remains independently available through gilrs::Mode.
fn start_ds4_touchpad_reader() -> Arc<AtomicBool> {
    const SONY_VENDOR_ID: u16 = 0x054C;
    // DualShock 4 v1, DualShock 4 v2, and Sony's USB wireless adapter.
    const DUALSHOCK4_PRODUCT_IDS: [u16; 3] = [0x05C4, 0x09CC, 0x0BA0];

    let pressed = Arc::new(AtomicBool::new(false));
    let thread_state = Arc::clone(&pressed);

    thread::spawn(move || {
        let mut api = match HidApi::new() {
            Ok(api) => api,
            Err(error) => {
                eprintln!("Could not initialize HID access for the DS4 touchpad: {error}");
                return;
            }
        };

        let mut device = None;

        loop {
            if device.is_none() {
                let _ = api.refresh_devices();
                device = api
                    .device_list()
                    .find(|info| {
                        info.vendor_id() == SONY_VENDOR_ID
                            && DUALSHOCK4_PRODUCT_IDS.contains(&info.product_id())
                    })
                    .and_then(|info| info.open_device(&api).ok());

                if device.is_none() {
                    thread_state.store(false, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(250));
                    continue;
                }
            }

            let mut report = [0u8; 96];
            let read_result = device
                .as_ref()
                .expect("HID device was just checked")
                .read_timeout(&mut report, 30);

            match read_result {
                Ok(read) => {
                    if let Some(is_pressed) = ds4_touchpad_click_from_report(&report[..read]) {
                        thread_state.store(is_pressed, Ordering::Relaxed);
                    }
                    // Ignore short or unrelated HID reports; don't invent a press.
                }
                Err(error) => {
                    eprintln!("DualShock 4 touchpad HID read stopped: {error}");
                    device = None;
                    thread_state.store(false, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(250));
                }
            }
        }
    });

    pressed
}

/// Reads an analog trigger. Most Windows backends report L2/R2 as an analog
/// *button* (not an axis), so check that first, then the Z axis, and finally
/// fall back to the digital pressed state so the trigger never reads as 0.
fn read_trigger(gamepad: &gilrs::Gamepad<'_>, button: PadButton, axis: Axis) -> f32 {
    let from_button = gamepad
        .button_data(button)
        .map(|data| data.value())
        .unwrap_or(0.0);
    let from_axis = trigger_value(gamepad.value(axis));
    let from_digital = if gamepad.is_pressed(button) { 1.0 } else { 0.0 };
    from_button.max(from_axis).max(from_digital).clamp(0.0, 1.0)
}

fn trigger_value(axis: f32) -> f32 {
    if axis < 0.0 {
        (axis + 1.0).clamp(0.0, 1.0)
    } else {
        axis.clamp(0.0, 1.0)
    }
}

fn mapping_targets() -> &'static [&'static str] {
    &[
        "Unmapped",
        "Xbox A",
        "Xbox B",
        "Xbox X",
        "Xbox Y",
        "Left Bumper",
        "Right Bumper",
        "Left Trigger",
        "Right Trigger",
        "D-pad Up",
        "D-pad Down",
        "D-pad Left",
        "D-pad Right",
        "Xbox Menu",
        "Xbox View",
        "Left Stick Click",
        "Right Stick Click",
    ]
}

fn mapped_button(binding: &str) -> u16 {
    match binding {
        "D-pad Up" | "D-pad up" => XButtons::UP,
        "D-pad Down" | "D-pad down" => XButtons::DOWN,
        "D-pad Left" | "D-pad left" => XButtons::LEFT,
        "D-pad Right" | "D-pad right" => XButtons::RIGHT,
        "Xbox A" | "Button 1" => XButtons::A,
        "Xbox B" | "Button 2" => XButtons::B,
        "Xbox X" | "Button 3" => XButtons::X,
        "Xbox Y" | "Button 4" => XButtons::Y,
        "Left Bumper" => XButtons::LB,
        "Right Bumper" => XButtons::RB,
        "Xbox Menu" | "Menu" => XButtons::START,
        "Xbox View" | "View" => XButtons::BACK,
        "Left Stick Click" | "Left Stick" => XButtons::LTHUMB,
        "Right Stick Click" | "Right Stick" => XButtons::RTHUMB,
        _ => 0,
    }
}

fn translate_controls(
    pressed: [bool; 18],
    left_trigger: f32,
    right_trigger: f32,
    bindings: &[String],
) -> XGamepad {
    let (mut buttons, mut output_left_trigger, mut output_right_trigger) = (0u16, 0.0f32, 0.0f32);
    for (index, is_pressed) in pressed.into_iter().enumerate() {
        if let Some(target) = bindings.get(index) {
            let pressed_value = if is_pressed { 1.0 } else { 0.0 };
            let value = match index {
                10 => left_trigger.clamp(0.0, 1.0),
                11 => right_trigger.clamp(0.0, 1.0),
                _ => pressed_value,
            };
            apply_mapping(
                target,
                value,
                &mut buttons,
                &mut output_left_trigger,
                &mut output_right_trigger,
            );
        }
    }
    XGamepad {
        buttons: XButtons::from(buttons),
        left_trigger: (output_left_trigger * 255.0).round() as u8,
        right_trigger: (output_right_trigger * 255.0).round() as u8,
        ..XGamepad::default()
    }
}

fn apply_mapping(
    target: &str,
    value: f32,
    buttons: &mut u16,
    left_trigger: &mut f32,
    right_trigger: &mut f32,
) {
    let value = value.clamp(0.0, 1.0);
    match target {
        "Left Trigger" => *left_trigger = left_trigger.max(value),
        "Right Trigger" => *right_trigger = right_trigger.max(value),
        "Unmapped" => {}
        _ if value >= 0.5 => *buttons |= mapped_button(target),
        _ => {}
    }
}

fn canonical_target(target: &str) -> String {
    match target {
        "Button 1" => "Xbox A",
        "Button 2" => "Xbox B",
        "Button 3" => "Xbox X",
        "Button 4" => "Xbox Y",
        "Menu" => "Xbox Menu",
        "View" => "Xbox View",
        "Left Stick" => "Left Stick Click",
        "Right Stick" => "Right Stick Click",
        other => other,
    }
    .to_owned()
}

fn migrate_profile_bindings(old: Vec<String>) -> Vec<String> {
    if old.len() == 14 {
        let mut migrated = default_bindings();
        for (index, binding) in old.iter().take(8).enumerate() {
            migrated[index] = canonical_target(binding);
        }
        migrated[10] = if old[8] == "Left Bumper" {
            "Left Trigger".into()
        } else {
            canonical_target(&old[8])
        };
        migrated[11] = if old[9] == "Right Bumper" {
            "Right Trigger".into()
        } else {
            canonical_target(&old[9])
        };
        migrated[12] = canonical_target(&old[10]);
        migrated[13] = canonical_target(&old[11]);
        migrated[14] = canonical_target(&old[12]);
        migrated[15] = canonical_target(&old[13]);
        return migrated;
    }

    let old_len = old.len();
    let mut bindings: Vec<String> = old.iter().map(|value| canonical_target(value)).collect();
    let defaults = default_bindings();

    // In the previous 17-control layout, index 16 was incorrectly labelled as
    // the touchpad even though gilrs::Mode actually reports the PS button.
    // Preserve any saved mapping at index 16 as the new touchpad mapping.
    if old_len == 17 {
        let previous_touchpad_binding = bindings[16].clone();
        bindings[16] = "Unmapped".into(); // New independent PS-button slot.
        bindings.push(previous_touchpad_binding);
    }

    bindings.resize_with(defaults.len(), String::new);
    for (index, binding) in bindings.iter_mut().enumerate() {
        if binding.is_empty() {
            *binding = defaults[index].clone();
        }
    }
    bindings
}

fn default_bindings() -> Vec<String> {
    [
        "D-pad Up",
        "D-pad Down",
        "D-pad Left",
        "D-pad Right",
        "Xbox A",
        "Xbox B",
        "Xbox X",
        "Xbox Y",
        "Left Bumper",
        "Right Bumper",
        "Left Trigger",
        "Right Trigger",
        "Xbox Menu",
        "Xbox View",
        "Left Stick Click",
        "Right Stick Click",
        "Unmapped", // PS button
        "Unmapped", // Touchpad click
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn load_profiles() -> Result<Option<Vec<Profile>>, String> {
    let path = profiles_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn profiles_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    base.map(|path| path.join("Padflow").join("profiles.json"))
        .ok_or_else(|| "Could not find a local settings folder".to_owned())
}

fn page_title(page: &str) -> &str {
    match page {
        "Button mapping" => "Button mapping",
        "Input test" => "Input test",
        "Profiles" => "Your profiles",
        "Compatibility" => "PlayStation controller support",
        "How to use" => "How to use DualLink",
        _ => "Controller setup",
    }
}

fn page_subtitle(page: &str) -> &str {
    match page {
        "Button mapping" => "Make the controls feel like yours.",
        "Input test" => "Explore stick positions and trigger range.",
        "Profiles" => "Keep a few layouts ready for different games.",
        "Compatibility" => "Use a DualShock 4 with games that expect an Xbox controller.",
        "How to use" => {
            "Simple setup steps, profile tips, and things to check if a game ignores input."
        }
        _ => "A simple home for your controller layout.",
    }
}

fn soft_shadow() -> egui::Shadow {
    egui::Shadow {
        offset: [0, 2],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(14),
    }
}

fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(egui::CornerRadius::same(16))
        .inner_margin(egui::Margin::same(18))
        .shadow(soft_shadow())
        .show(ui, content);
}

fn primary_button(label: &str, min_width: f32) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label)
            .size(13.0)
            .strong()
            .color(Color32::WHITE),
    )
    .fill(BLUE)
    .stroke(Stroke::NONE)
    .corner_radius(egui::CornerRadius::same(10))
    .min_size(Vec2::new(min_width, 38.0))
}

fn secondary_button(label: &str, min_width: f32) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(label).size(13.0).strong().color(BLUE))
        .fill(Color32::from_rgb(237, 241, 255))
        .stroke(Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(10))
        .min_size(Vec2::new(min_width, 38.0))
}

/// An on/off switch. Returns the click response; the caller decides what to do with it.
fn toggle_switch(ui: &mut egui::Ui, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(46.0, 26.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, on);
        let enabled = ui.is_enabled();
        let track = Color32::from_rgb(205, 212, 224).lerp_to_gamma(GREEN, t);
        let track = if enabled { track } else { track.gamma_multiply(0.55) };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(13), track);
        let knob = Pos2::new(
            egui::lerp((rect.left() + 13.0)..=(rect.right() - 13.0), t),
            rect.center().y,
        );
        ui.painter()
            .circle_filled(knob + Vec2::new(0.0, 1.0), 10.5, Color32::from_black_alpha(22));
        ui.painter().circle_filled(knob, 10.0, Color32::WHITE);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One row of the sidebar navigation: painted by hand so it can show a hover and an accent bar.
fn nav_item(ui: &mut egui::Ui, marker: &str, label: &str, selected: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(10),
            if selected {
                Color32::from_rgb(48, 66, 99)
            } else {
                Color32::from_rgb(33, 47, 74)
            },
        );
    }
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.left_top() + Vec2::new(0.0, 11.0), Vec2::new(3.0, 20.0)),
            egui::CornerRadius::same(2),
            Color32::from_rgb(120, 150, 255),
        );
    }
    let color = if selected {
        Color32::WHITE
    } else {
        Color32::from_rgb(180, 193, 214)
    };
    ui.painter().text(
        rect.left_center() + Vec2::new(18.0, 0.0),
        egui::Align2::LEFT_CENTER,
        marker,
        egui::FontId::proportional(14.0),
        color,
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(46.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        color,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// App-wide look: soft rounded controls, blue accents, white popups.
fn apply_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 7.0);
    style.spacing.interact_size.y = 32.0;
    let radius = egui::CornerRadius::same(9);
    let visuals = &mut style.visuals;
    visuals.panel_fill = BG;
    visuals.window_fill = Color32::WHITE;
    visuals.extreme_bg_color = Color32::WHITE;
    visuals.faint_bg_color = Color32::from_rgb(246, 248, 252);
    visuals.hyperlink_color = BLUE;
    visuals.selection.bg_fill = Color32::from_rgb(214, 224, 255);
    visuals.selection.stroke = Stroke::new(1.0_f32, BLUE);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, INK);
    for (widget, fill, stroke_color) in [
        (&mut visuals.widgets.inactive, Color32::from_rgb(238, 242, 250), Color32::from_rgb(222, 228, 240)),
        (&mut visuals.widgets.hovered, Color32::from_rgb(226, 234, 252), Color32::from_rgb(170, 190, 250)),
        (&mut visuals.widgets.active, Color32::from_rgb(210, 222, 252), BLUE),
        (&mut visuals.widgets.open, Color32::from_rgb(226, 234, 252), Color32::from_rgb(170, 190, 250)),
    ] {
        widget.weak_bg_fill = fill;
        widget.bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0_f32, stroke_color);
        widget.fg_stroke = Stroke::new(1.0_f32, INK);
        widget.corner_radius = radius;
        widget.expansion = 0.0;
    }
    ctx.set_style(style);
}

fn metric_card(ui: &mut egui::Ui, title: &str, value: &str, detail: &str, accent: Color32) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("●").size(10.0).color(accent));
            ui.label(egui::RichText::new(title).size(9.0).strong().color(MUTED));
        });
        ui.add_space(7.0);
        ui.label(egui::RichText::new(value).size(19.0).strong().color(INK));
        ui.label(egui::RichText::new(detail).size(10.0).color(MUTED));
    });
}

fn section_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(title).size(15.0).strong().color(INK));
        ui.label(egui::RichText::new(subtitle).size(10.5).color(MUTED));
    });
}

fn mapping_row(ui: &mut egui::Ui, index: usize, action: &str, binding: Option<&mut String>) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(action).size(11.5).color(INK));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(binding) = binding {
                egui::ComboBox::from_id_salt(("mapping-target", index))
                    .selected_text(binding.as_str())
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for target in mapping_targets() {
                            ui.selectable_value(binding, (*target).to_owned(), *target);
                        }
                    });
            }
        });
    });
    ui.add_space(5.0);
    ui.separator();
}

fn binding_row(ui: &mut egui::Ui, action: &str, value: &str, button: Option<&str>) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(action).size(11.5).color(INK));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(button) = button {
                clicked = ui.small_button(button).clicked();
                ui.label(egui::RichText::new(value).size(10.5).strong().color(BLUE));
            } else {
                ui.label(egui::RichText::new(value).size(10.5).strong().color(BLUE));
            }
        });
    });
    ui.add_space(7.0);
    ui.separator();
    ui.add_space(5.0);
    clicked
}

fn pill(ui: &mut egui::Ui, label: &str, background: Color32, foreground: Color32) {
    egui::Frame::new()
        .fill(background)
        .corner_radius(egui::CornerRadius::same(20))
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 5,
            bottom: 5,
        })
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(9.0)
                    .strong()
                    .color(foreground),
            );
        });
}

fn stick_controls(ui: &mut egui::Ui, label: &str, x: &mut f32, y: &mut f32) {
    ui.label(egui::RichText::new(label).size(13.0).strong().color(INK));
    ui.add_space(7.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("X").size(10.0).strong().color(MUTED));
        ui.add(egui::Slider::new(x, -1.0..=1.0).show_value(false));
        ui.label(egui::RichText::new("Y").size(10.0).strong().color(MUTED));
        ui.add(egui::Slider::new(y, -1.0..=1.0).show_value(false));
    });
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("Horizontal  {:+.2}     Vertical  {:+.2}", *x, *y))
                .size(10.0)
                .color(MUTED),
        );
    });
}

#[derive(Clone, Copy, Default)]
struct PadView {
    left_x: f32,
    left_y: f32,
    right_x: f32,
    right_y: f32,
    trigger_left: f32,
    trigger_right: f32,
    /// Same order as the `sources` list in `poll_controller`:
    /// 0-3 D-pad up/down/left/right, 4 Cross, 5 Circle, 6 Square, 7 Triangle,
    /// 8 L1, 9 R1, 10 L2, 11 R2, 12 Options, 13 Share, 14 L3, 15 R3, 16 PS button, 17 touchpad click.
    pressed: [bool; 18],
}

/// Splits a (possibly concave) polygon into triangles by ear clipping.
fn triangulate(points: &[Pos2]) -> Vec<u32> {
    let mut order: Vec<usize> = (0..points.len()).collect();
    let mut area = 0.0_f32;
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        area += a.x * b.y - b.x * a.y;
    }
    if area < 0.0 {
        order.reverse();
    }
    let inside = |p: Pos2, a: Pos2, b: Pos2, c: Pos2| {
        let side = |u: Pos2, v: Pos2, w: Pos2| (v.x - u.x) * (w.y - u.y) - (v.y - u.y) * (w.x - u.x);
        side(a, b, p) >= 0.0 && side(b, c, p) >= 0.0 && side(c, a, p) >= 0.0
    };
    let mut triangles = Vec::new();
    while order.len() > 3 {
        let count = order.len();
        let mut clipped = false;
        for i in 0..count {
            let (ia, ib, ic) = (order[(i + count - 1) % count], order[i], order[(i + 1) % count]);
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            if (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x) <= 0.0 {
                continue;
            }
            if order
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && inside(points[j], a, b, c))
            {
                continue;
            }
            triangles.extend([ia as u32, ib as u32, ic as u32]);
            order.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if order.len() == 3 {
        triangles.extend(order.iter().map(|&i| i as u32));
    }
    triangles
}

/// Smooth DualShock 4 silhouette in a 400-wide design space, built once.
fn body_shape() -> &'static (Vec<Pos2>, Vec<u32>) {
    static BODY: std::sync::OnceLock<(Vec<Pos2>, Vec<u32>)> = std::sync::OnceLock::new();
    BODY.get_or_init(|| {
        // Right half, from top center clockwise down to bottom center.
        let right = [
            (200.0_f32, 92.0_f32),
            (252.0, 90.0),
            (292.0, 93.0),
            (320.0, 110.0),
            (338.0, 144.0),
            (348.0, 184.0),
            (344.0, 216.0),
            (328.0, 236.0),
            (304.0, 236.0),
            (284.0, 216.0),
            (268.0, 200.0),
            (240.0, 196.0),
            (200.0, 198.0),
        ];
        let mut control: Vec<Pos2> = right.iter().map(|&(x, y)| Pos2::new(x, y)).collect();
        for &(x, y) in right[1..right.len() - 1].iter().rev() {
            control.push(Pos2::new(400.0 - x, y));
        }
        let n = control.len();
        let mut points = Vec::new();
        for i in 0..n {
            let (p0, p1, p2, p3) = (
                control[(i + n - 1) % n],
                control[i],
                control[(i + 1) % n],
                control[(i + 2) % n],
            );
            for step in 0..8 {
                let t = step as f32 / 8.0;
                let (t2, t3) = (t * t, t * t * t);
                let spline = |a: f32, b: f32, c: f32, d: f32| {
                    0.5 * (2.0 * b
                        + (-a + c) * t
                        + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                        + (-a + 3.0 * b - 3.0 * c + d) * t3)
                };
                points.push(Pos2::new(
                    spline(p0.x, p1.x, p2.x, p3.x),
                    spline(p0.y, p1.y, p2.y, p3.y),
                ));
            }
        }
        let triangles = triangulate(&points);
        (points, triangles)
    })
}

fn draw_controller(ui: &mut egui::Ui, view: &PadView) {
    let width = ui.available_width().max(260.0);
    let scale = (width / 400.0).min(1.15);
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, 182.0 * scale), Sense::hover());
    let painter = ui.painter_at(area);
    let origin = Pos2::new(area.center().x - 200.0 * scale, area.top());
    let p = |x: f32, y: f32| origin + Vec2::new(x, y - 60.0) * scale;

    let outline = Color32::from_rgb(10, 13, 20);
    let shell_light = Color32::from_rgb(58, 66, 86);
    let well = Color32::from_rgb(20, 24, 33);
    let idle = Color32::from_rgb(96, 108, 130);
    let lit = BLUE;

    // Triggers (L2/R2): small tabs behind the shoulders that fill as they are pulled.
    for (x0, value) in [(100.0_f32, view.trigger_left), (264.0_f32, view.trigger_right)] {
        let rect = Rect::from_min_max(p(x0, 72.0), p(x0 + 36.0, 95.0));
        let radius = egui::CornerRadius {
            nw: (14.0 * scale) as u8,
            ne: (14.0 * scale) as u8,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(rect.expand(1.5), radius, outline);
        painter.rect_filled(rect, radius, shell_light);
        let fill = rect.height() * value.clamp(0.0, 1.0);
        if fill > 0.5 {
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - fill), rect.right_bottom()),
                radius,
                lit,
            );
        }
    }

    // Body: smooth silhouette with a light-to-dark vertical gradient and a crisp outline.
    let (design_points, triangles) = body_shape();
    let screen: Vec<Pos2> = design_points.iter().map(|q| p(q.x, q.y)).collect();
    let mut mesh = egui::Mesh::default();
    for (q, s) in design_points.iter().zip(&screen) {
        let t = ((q.y - 90.0) / 146.0).clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| (a + (b - a) * t) as u8;
        mesh.colored_vertex(*s, Color32::from_rgb(mix(56.0, 30.0), mix(64.0, 35.0), mix(84.0, 48.0)));
    }
    mesh.indices = triangles.clone();
    painter.add(egui::Shape::mesh(mesh));
    painter.add(egui::Shape::closed_line(screen, Stroke::new(2.0, outline)));

    // Bumpers (L1/R1): strips along the shoulders.
    for (mirrored, down) in [(true, view.pressed[8]), (false, view.pressed[9])] {
        let x = |v: f32| if mirrored { 400.0 - v } else { v };
        let strip = vec![p(x(246.0), 96.0), p(x(270.0), 96.0), p(x(288.0), 98.0), p(x(300.0), 104.0)];
        painter.add(egui::Shape::line(strip.clone(), Stroke::new(8.0 * scale, outline)));
        painter.add(egui::Shape::line(
            strip,
            Stroke::new(5.5 * scale, if down { lit } else { shell_light }),
        ));
    }

    // Light bar glow and touchpad.
    painter.rect_filled(
        Rect::from_min_max(p(166.0, 95.0), p(234.0, 103.0)),
        egui::CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(77, 111, 240, 55),
    );
    painter.rect_filled(
        Rect::from_min_max(p(174.0, 97.0), p(226.0, 100.0)),
        egui::CornerRadius::same(2),
        lit,
    );
    let pad = Rect::from_min_max(p(160.0, 106.0), p(240.0, 146.0));
    let pad_radius = egui::CornerRadius::same((10.0 * scale) as u8);
    painter.rect_filled(pad.expand(1.5), pad_radius, outline);
    painter.rect_filled(pad, pad_radius, if view.pressed[17] { lit } else { shell_light });

    // Share (left) and Options (right).
    for (x, down) in [(144.0_f32, view.pressed[13]), (256.0_f32, view.pressed[12])] {
        let rect = Rect::from_center_size(p(x, 110.0), Vec2::new(6.0, 12.0) * scale);
        painter.rect_filled(rect.expand(1.2), egui::CornerRadius::same(4), outline);
        painter.rect_filled(rect, egui::CornerRadius::same(3), if down { lit } else { idle });
    }

    // D-pad.
    let dpad = p(116.0, 134.0);
    painter.rect_filled(
        Rect::from_center_size(dpad, Vec2::splat(18.0 * scale)),
        egui::CornerRadius::same(2),
        well,
    );
    for (offset, size, index) in [
        (Vec2::new(0.0, -14.0), Vec2::new(15.0, 19.0), 0),
        (Vec2::new(0.0, 14.0), Vec2::new(15.0, 19.0), 1),
        (Vec2::new(-14.0, 0.0), Vec2::new(19.0, 15.0), 2),
        (Vec2::new(14.0, 0.0), Vec2::new(19.0, 15.0), 3),
    ] {
        let rect = Rect::from_center_size(dpad + offset * scale, size * scale);
        painter.rect_filled(rect.expand(1.2), egui::CornerRadius::same(4), outline);
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(3),
            if view.pressed[index] { lit } else { well },
        );
    }

    // Face buttons with PlayStation symbols.
    let faces = p(284.0, 134.0);
    for (offset, index, color, symbol) in [
        (Vec2::new(0.0, -21.0), 7, Color32::from_rgb(76, 205, 172), 't'),
        (Vec2::new(21.0, 0.0), 5, Color32::from_rgb(235, 98, 112), 'o'),
        (Vec2::new(0.0, 21.0), 4, Color32::from_rgb(98, 150, 242), 'x'),
        (Vec2::new(-21.0, 0.0), 6, Color32::from_rgb(226, 140, 192), 's'),
    ] {
        let c = faces + offset * scale;
        let down = view.pressed[index];
        painter.circle_filled(c, 12.5 * scale, outline);
        painter.circle_filled(c, 11.0 * scale, if down { color } else { well });
        let stroke = Stroke::new(1.8 * scale, if down { Color32::WHITE } else { color });
        let k = scale;
        match symbol {
            't' => {
                painter.add(egui::Shape::closed_line(
                    vec![
                        c + Vec2::new(0.0, -5.5 * k),
                        c + Vec2::new(5.0 * k, 3.5 * k),
                        c + Vec2::new(-5.0 * k, 3.5 * k),
                    ],
                    stroke,
                ));
            }
            'o' => {
                painter.circle_stroke(c, 5.0 * k, stroke);
            }
            'x' => {
                painter.line_segment([c + Vec2::new(-4.5 * k, -4.5 * k), c + Vec2::new(4.5 * k, 4.5 * k)], stroke);
                painter.line_segment([c + Vec2::new(-4.5 * k, 4.5 * k), c + Vec2::new(4.5 * k, -4.5 * k)], stroke);
            }
            _ => {
                painter.rect_stroke(
                    Rect::from_center_size(c, Vec2::splat(9.0 * k)),
                    egui::CornerRadius::same(1),
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
    }

    // PS button.
    painter.circle_filled(p(200.0, 168.0), 8.0 * scale, outline);
    painter.circle_filled(p(200.0, 168.0), 6.5 * scale, if view.pressed[16] { lit } else { idle });

    // Analog sticks move live; the ring lights up on L3/R3 click.
    for (center, x, y, down) in [
        (p(160.0, 172.0), view.left_x, view.left_y, view.pressed[14]),
        (p(240.0, 172.0), view.right_x, view.right_y, view.pressed[15]),
    ] {
        painter.circle_filled(center, 22.0 * scale, outline);
        painter.circle_filled(center, 20.0 * scale, well);
        let cap = center + Vec2::new(x.clamp(-1.0, 1.0), -y.clamp(-1.0, 1.0)) * (7.0 * scale);
        painter.circle_filled(cap, 14.0 * scale, shell_light);
        painter.circle_stroke(
            cap,
            14.0 * scale,
            Stroke::new(if down { 2.5 * scale } else { 1.0 }, if down { lit } else { idle }),
        );
        painter.circle_stroke(cap, 8.0 * scale, Stroke::new(1.0, well));
    }
}

#[derive(Clone, Copy)]
enum Pill<'a> {
    Text(&'a str),
    Symbol(char, Color32),
}

/// PlayStation face-button symbol: 't' triangle, 'o' circle, 'x' cross, anything else square.
fn draw_symbol(painter: &egui::Painter, c: Pos2, symbol: char, k: f32, stroke: Stroke) {
    match symbol {
        't' => {
            painter.add(egui::Shape::closed_line(
                vec![
                    c + Vec2::new(0.0, -5.5 * k),
                    c + Vec2::new(5.0 * k, 3.5 * k),
                    c + Vec2::new(-5.0 * k, 3.5 * k),
                ],
                stroke,
            ));
        }
        'o' => {
            painter.circle_stroke(c, 5.0 * k, stroke);
        }
        'x' => {
            painter.line_segment([c + Vec2::new(-4.5 * k, -4.5 * k), c + Vec2::new(4.5 * k, 4.5 * k)], stroke);
            painter.line_segment([c + Vec2::new(-4.5 * k, 4.5 * k), c + Vec2::new(4.5 * k, -4.5 * k)], stroke);
        }
        _ => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(9.0 * k)),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Middle,
            );
        }
    }
}

/// One callout label: a small pill naming the PS4 control plus the Xbox output it sends.
/// `side` -1 puts the text left of `edge`, +1 right of it, 0 centres the row on it.
#[allow(clippy::too_many_arguments)]
fn callout_row(
    painter: &egui::Painter,
    edge: f32,
    y: f32,
    side: i8,
    pill: Pill,
    text: &str,
    color: Color32,
    scale: f32,
) {
    let font = egui::FontId::proportional((11.5 * scale).clamp(9.5, 15.0));
    let pill_font = egui::FontId::proportional((8.5 * scale).clamp(7.5, 12.0));
    let text_galley = painter.layout_no_wrap(text.to_uppercase(), font, color);
    let pill_galley = match pill {
        Pill::Text(label) => Some(painter.layout_no_wrap(label.to_owned(), pill_font, color)),
        Pill::Symbol(..) => None,
    };
    let pill_w = pill_galley
        .as_ref()
        .map_or(24.0 * scale, |g| g.size().x + 12.0 * scale);
    let pill_h = 16.0 * scale;
    let gap = 7.0 * scale;
    let text_w = text_galley.size().x;
    let total = text_w + gap + pill_w;
    let start = if side < 0 {
        edge - total
    } else if side > 0 {
        edge
    } else {
        edge - total / 2.0
    };
    let (pill_x, text_x) = if side < 0 {
        (start + text_w + gap, start)
    } else {
        (start, start + pill_w + gap)
    };
    let pill_rect = Rect::from_min_size(
        Pos2::new(pill_x, y - pill_h / 2.0),
        Vec2::new(pill_w, pill_h),
    );
    let outline = match pill {
        Pill::Symbol(_, tint) => tint,
        Pill::Text(_) => color,
    };
    painter.rect_stroke(
        pill_rect,
        egui::CornerRadius::same((pill_h / 2.0) as u8),
        Stroke::new(1.2_f32, outline),
        egui::StrokeKind::Inside,
    );
    match (pill, pill_galley) {
        (Pill::Symbol(symbol, tint), _) => {
            draw_symbol(painter, pill_rect.center(), symbol, 0.95 * scale.max(0.7), Stroke::new(1.4_f32, tint));
        }
        (_, Some(galley)) => {
            let pos = pill_rect.center() - galley.size() / 2.0;
            painter.galley(pos, galley, color);
        }
        _ => {}
    }
    painter.galley(Pos2::new(text_x, y - text_galley.size().y / 2.0), text_galley, color);
}

/// Large line-art controller with callouts showing the Xbox output mapped to each PS4 control.
fn draw_controller_map(ui: &mut egui::Ui, view: &PadView, bindings: &[String]) {
    let width = ui.available_width().max(520.0);
    let scale = (width / 860.0).min(1.25);
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, 440.0 * scale), Sense::hover());
    let painter = ui.painter_at(area);
    painter.rect_filled(area, egui::CornerRadius::same(12), Color32::from_rgb(13, 16, 22));

    let origin = Pos2::new(area.center().x - 430.0 * scale, area.top());
    let p = |x: f32, y: f32| origin + Vec2::new(x, y) * scale;
    // Controller space (the 400-wide design used by the compact preview) -> diagram space.
    let c = |x: f32, y: f32| p(430.0 + (x - 200.0) * 1.25, 86.0 + (y - 66.0) * 1.25);
    let u = scale * 1.25;

    let line = Color32::from_rgb(214, 221, 233);
    let dim = Color32::from_rgb(140, 151, 170);
    let label = Color32::from_rgb(222, 228, 238);
    let lit = Color32::from_rgb(110, 145, 255);
    let well = Color32::from_rgb(16, 19, 26);
    let thin = Stroke::new(1.4 * scale, line);
    let value = |index: usize| -> String {
        bindings.get(index).cloned().unwrap_or_else(|| "Unmapped".to_owned())
    };
    let tint = |index: usize| if view.pressed[index] { lit } else { label };

    // Trigger tabs (L2/R2) fill as they are pulled.
    for (x0, level, name) in [(100.0_f32, view.trigger_left, "L2"), (264.0_f32, view.trigger_right, "R2")] {
        let rect = Rect::from_min_max(c(x0, 70.0), c(x0 + 36.0, 95.0));
        let r = (14.0 * u) as u8;
        let radius = egui::CornerRadius { nw: r, ne: r, sw: 0, se: 0 };
        painter.rect_filled(rect, radius, well);
        let fill = rect.height() * level.clamp(0.0, 1.0);
        if fill > 0.5 {
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - fill), rect.right_bottom()),
                radius,
                lit,
            );
        }
        painter.rect_stroke(rect, radius, thin, egui::StrokeKind::Inside);
        painter.text(
            Pos2::new(rect.center().x, rect.top() + rect.height() * 0.4),
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::proportional((8.5 * scale).clamp(7.0, 11.0)),
            if level > 0.5 { Color32::WHITE } else { dim },
        );
    }

    // Body: dark fill, bright outline and a thin inner rim.
    let (design_points, triangles) = body_shape();
    let screen: Vec<Pos2> = design_points.iter().map(|q| c(q.x, q.y)).collect();
    let mut mesh = egui::Mesh::default();
    for (q, s) in design_points.iter().zip(&screen) {
        let t = ((q.y - 90.0) / 146.0).clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| (a + (b - a) * t) as u8;
        mesh.colored_vertex(*s, Color32::from_rgb(mix(30.0, 15.0), mix(36.0, 18.0), mix(48.0, 25.0)));
    }
    mesh.indices = triangles.clone();
    painter.add(egui::Shape::mesh(mesh));
    let rim: Vec<Pos2> = design_points
        .iter()
        .map(|q| c(200.0 + (q.x - 200.0) * 0.965, 160.0 + (q.y - 160.0) * 0.94))
        .collect();
    painter.add(egui::Shape::closed_line(rim, Stroke::new(1.0_f32, Color32::from_rgb(60, 68, 86))));
    painter.add(egui::Shape::closed_line(screen, Stroke::new(1.7 * scale, line)));

    // Bumpers (L1 left, R1 right): hollow strips along the shoulders.
    for (mirrored, index) in [(true, 8_usize), (false, 9_usize)] {
        let x = |v: f32| if mirrored { 400.0 - v } else { v };
        let strip: Vec<Pos2> = [(246.0_f32, 96.0_f32), (270.0, 96.0), (288.0, 98.0), (300.0, 104.0)]
            .iter()
            .map(|&(vx, vy)| c(x(vx), vy))
            .collect();
        painter.add(egui::Shape::line(strip.clone(), Stroke::new(9.0 * u, line)));
        painter.add(egui::Shape::line(
            strip,
            Stroke::new(7.0 * u, if view.pressed[index] { lit } else { well }),
        ));
    }

    // Light bar, touchpad (with dot texture), Share and Options.
    painter.line_segment(
        [c(172.0, 98.0), c(228.0, 98.0)],
        Stroke::new(2.0 * scale, Color32::from_rgb(70, 100, 220)),
    );
    let pad = Rect::from_min_max(c(160.0, 106.0), c(240.0, 146.0));
    painter.rect_filled(pad, egui::CornerRadius::same((10.0 * scale) as u8), if view.pressed[17] { lit } else { well });
    painter.rect_stroke(pad, egui::CornerRadius::same((10.0 * scale) as u8), thin, egui::StrokeKind::Inside);
    for row in 0..8 {
        for col in 0..17 {
            painter.circle_filled(
                c(168.0 + col as f32 * 4.5, 113.0 + row as f32 * 4.0),
                0.75 * scale,
                Color32::from_rgb(110, 121, 142),
            );
        }
    }
    for (x, index) in [(144.0_f32, 13_usize), (256.0_f32, 12_usize)] {
        let rect = Rect::from_center_size(c(x, 110.0), Vec2::new(6.0, 12.0) * u);
        painter.rect_filled(rect, egui::CornerRadius::same(3), if view.pressed[index] { lit } else { well });
        painter.rect_stroke(rect, egui::CornerRadius::same(3), thin, egui::StrokeKind::Inside);
    }

    // D-pad: a plus shape with an arrow in each arm.
    let dpad = c(116.0, 134.0);
    let arms = [
        (Vec2::new(0.0, -14.0), Vec2::new(15.0, 19.0), 0_usize, Vec2::new(0.0, -1.0)),
        (Vec2::new(0.0, 14.0), Vec2::new(15.0, 19.0), 1, Vec2::new(0.0, 1.0)),
        (Vec2::new(-14.0, 0.0), Vec2::new(19.0, 15.0), 2, Vec2::new(-1.0, 0.0)),
        (Vec2::new(14.0, 0.0), Vec2::new(19.0, 15.0), 3, Vec2::new(1.0, 0.0)),
    ];
    let centre_rect = Rect::from_center_size(dpad, Vec2::splat(18.0 * u));
    painter.rect_filled(centre_rect.expand(1.4 * scale), egui::CornerRadius::same(2), line);
    for (offset, size, _, _) in arms {
        let rect = Rect::from_center_size(dpad + offset * u, size * u);
        painter.rect_filled(rect.expand(1.4 * scale), egui::CornerRadius::same(4), line);
    }
    painter.rect_filled(centre_rect, egui::CornerRadius::same(2), well);
    for (offset, size, index, dir) in arms {
        let rect = Rect::from_center_size(dpad + offset * u, size * u);
        painter.rect_filled(rect, egui::CornerRadius::same(3), if view.pressed[index] { lit } else { well });
        let middle = rect.center();
        let normal = Vec2::new(-dir.y, dir.x);
        let base = middle - dir * (2.5 * u);
        painter.add(egui::Shape::convex_polygon(
            vec![middle + dir * (5.5 * u), base + normal * (4.0 * u), base - normal * (4.0 * u)],
            if view.pressed[index] { Color32::WHITE } else { dim },
            Stroke::NONE,
        ));
    }

    // Face buttons.
    let faces = c(284.0, 134.0);
    let face_colors = [
        (Vec2::new(0.0, -21.0), 7_usize, Color32::from_rgb(76, 205, 172), 't'),
        (Vec2::new(21.0, 0.0), 5, Color32::from_rgb(235, 98, 112), 'o'),
        (Vec2::new(0.0, 21.0), 4, Color32::from_rgb(98, 150, 242), 'x'),
        (Vec2::new(-21.0, 0.0), 6, Color32::from_rgb(226, 140, 192), 's'),
    ];
    for (offset, index, color, symbol) in face_colors {
        let center = faces + offset * u;
        let down = view.pressed[index];
        painter.circle_filled(center, 11.5 * u, if down { color } else { well });
        painter.circle_stroke(center, 11.5 * u, thin);
        draw_symbol(&painter, center, symbol, u, Stroke::new(1.6 * scale, if down { Color32::WHITE } else { line }));
    }

    // PS button and speaker dots.
    painter.circle_filled(c(200.0, 168.0), 6.5 * u, if view.pressed[16] { lit } else { well });
    painter.circle_stroke(c(200.0, 168.0), 7.5 * u, thin);
    for row in 0..2 {
        for col in 0..3 {
            painter.circle_filled(
                c(195.0 + col as f32 * 5.0, 154.0 + row as f32 * 4.0),
                0.9 * scale,
                Color32::from_rgb(110, 121, 142),
            );
        }
    }

    // Analog sticks (live) with a click highlight for L3/R3.
    let sticks = [
        (c(160.0, 172.0), view.left_x, view.left_y, view.pressed[14]),
        (c(240.0, 172.0), view.right_x, view.right_y, view.pressed[15]),
    ];
    for (center, x, y, down) in sticks {
        painter.circle_filled(center, 22.0 * u, well);
        painter.circle_stroke(center, 22.0 * u, thin);
        let cap = center + Vec2::new(x.clamp(-1.0, 1.0), -y.clamp(-1.0, 1.0)) * (7.0 * u);
        painter.circle_filled(cap, 15.0 * u, if down { lit } else { well });
        painter.circle_stroke(cap, 15.0 * u, thin);
        painter.circle_stroke(cap, 9.0 * u, Stroke::new(1.0_f32, dim));
    }

    // ---- Callouts ----
    let leader = Stroke::new(1.1 * scale, dim);
    let dot = |at: Pos2| painter.circle_filled(at, 2.4 * scale, label);
    let (left_edge, right_edge) = (p(240.0, 0.0).x, p(620.0, 0.0).x);
    let inset = 6.0 * scale;

    // L2 / R2: elbow lines from above.
    let row_y = p(0.0, 74.0).y;
    let l2 = c(118.0, 70.0);
    painter.add(egui::Shape::line(vec![Pos2::new(left_edge + inset, row_y), Pos2::new(l2.x, row_y), l2], leader));
    dot(l2);
    callout_row(&painter, left_edge, row_y, -1, Pill::Text("L2"), &value(10), tint(10), scale);
    let r2 = c(282.0, 70.0);
    painter.add(egui::Shape::line(vec![Pos2::new(right_edge - inset, row_y), Pos2::new(r2.x, row_y), r2], leader));
    dot(r2);
    callout_row(&painter, right_edge, row_y, 1, Pill::Text("R2"), &value(11), tint(11), scale);

    // L1 / R1: straight lines to the bumpers.
    let l1 = c(130.0, 97.0);
    painter.add(egui::Shape::line(vec![Pos2::new(left_edge + inset, l1.y), l1], leader));
    dot(l1);
    callout_row(&painter, left_edge, l1.y, -1, Pill::Text("L1"), &value(8), tint(8), scale);
    let r1 = c(270.0, 97.0);
    painter.add(egui::Shape::line(vec![Pos2::new(right_edge - inset, r1.y), r1], leader));
    dot(r1);
    callout_row(&painter, right_edge, r1.y, 1, Pill::Text("R1"), &value(9), tint(9), scale);

    // Touchpad click: label at the top centre with a line straight down to the pad.
    let pad_top = c(200.0, 106.0);
    let touch_y = p(0.0, 16.0).y;
    painter.add(egui::Shape::line(
        vec![Pos2::new(pad_top.x, touch_y + 12.0 * scale), pad_top],
        leader,
    ));
    dot(pad_top);
    callout_row(&painter, pad_top.x, touch_y, 0, Pill::Text("TOUCHPAD"), &value(17), tint(17), scale);

    // Share / Options: labels sit a little outward so they clear the touchpad line.
    for (x, index, name, shift) in [
        (144.0_f32, 13_usize, "SHARE", -22.0_f32),
        (256.0_f32, 12_usize, "OPTIONS", 22.0_f32),
    ] {
        let at = c(x, 104.0);
        let y = p(0.0, 46.0).y;
        painter.add(egui::Shape::line(vec![Pos2::new(at.x, y + 12.0 * scale), at], leader));
        dot(at);
        callout_row(&painter, at.x + shift * scale, y, 0, Pill::Text(name), &value(index), tint(index), scale);
    }

    // D-pad bracket on the left.
    let row_gap = 19.0 * scale;
    let bracket_x = left_edge + 14.0 * scale;
    let serif = 5.0 * scale;
    let dpad_rows = [(0_usize, "UP"), (1, "DOWN"), (2, "LEFT"), (3, "RIGHT")];
    let top = dpad.y - 1.5 * row_gap;
    let bottom = dpad.y + 1.5 * row_gap;
    painter.add(egui::Shape::line(
        vec![
            Pos2::new(bracket_x - serif, top - 7.0 * scale),
            Pos2::new(bracket_x, top - 7.0 * scale),
            Pos2::new(bracket_x, bottom + 7.0 * scale),
            Pos2::new(bracket_x - serif, bottom + 7.0 * scale),
        ],
        leader,
    ));
    let dpad_anchor = Pos2::new(dpad.x - 24.0 * u, dpad.y);
    painter.line_segment([Pos2::new(bracket_x, dpad.y), dpad_anchor], leader);
    dot(dpad_anchor);
    for (i, (index, name)) in dpad_rows.iter().enumerate() {
        let y = top + i as f32 * row_gap;
        callout_row(&painter, bracket_x - 9.0 * scale, y, -1, Pill::Text(name), &value(*index), tint(*index), scale);
    }

    // Face-button bracket on the right.
    let bracket_x = right_edge - 14.0 * scale;
    let top = faces.y - 1.5 * row_gap;
    let bottom = faces.y + 1.5 * row_gap;
    painter.add(egui::Shape::line(
        vec![
            Pos2::new(bracket_x + serif, top - 7.0 * scale),
            Pos2::new(bracket_x, top - 7.0 * scale),
            Pos2::new(bracket_x, bottom + 7.0 * scale),
            Pos2::new(bracket_x + serif, bottom + 7.0 * scale),
        ],
        leader,
    ));
    let faces_anchor = Pos2::new(faces.x + 33.0 * u, faces.y);
    painter.line_segment([Pos2::new(bracket_x, faces.y), faces_anchor], leader);
    dot(faces_anchor);
    let face_rows = [
        (4_usize, 'x', Color32::from_rgb(98, 150, 242)),
        (5, 'o', Color32::from_rgb(235, 98, 112)),
        (6, 's', Color32::from_rgb(226, 140, 192)),
        (7, 't', Color32::from_rgb(76, 205, 172)),
    ];
    for (i, (index, symbol, color)) in face_rows.iter().enumerate() {
        let y = top + i as f32 * row_gap;
        callout_row(&painter, bracket_x + 9.0 * scale, y, 1, Pill::Symbol(*symbol, *color), &value(*index), tint(*index), scale);
    }

    // Sticks: lines run down and out to labels at the bottom.
    let line_y = p(0.0, 371.0).y;
    let left_stick = sticks[0].0;
    let right_stick = sticks[1].0;
    let left_bottom = Pos2::new(left_stick.x, left_stick.y + 22.0 * u);
    painter.add(egui::Shape::line(
        vec![left_bottom, Pos2::new(left_bottom.x, line_y), Pos2::new(left_edge + inset, line_y)],
        leader,
    ));
    dot(left_bottom);
    callout_row(&painter, left_edge, line_y - 15.0 * scale, -1, Pill::Text("L"), "Left stick", dim, scale);
    callout_row(&painter, left_edge, line_y + 15.0 * scale, -1, Pill::Text("L3"), &value(14), tint(14), scale);
    let right_bottom = Pos2::new(right_stick.x, right_stick.y + 22.0 * u);
    painter.add(egui::Shape::line(
        vec![right_bottom, Pos2::new(right_bottom.x, line_y), Pos2::new(right_edge - inset, line_y)],
        leader,
    ));
    dot(right_bottom);
    callout_row(&painter, right_edge, line_y - 15.0 * scale, 1, Pill::Text("R"), "Right stick", dim, scale);
    callout_row(&painter, right_edge, line_y + 15.0 * scale, 1, Pill::Text("R3"), &value(15), tint(15), scale);

    // PS button: label at the bottom centre, line straight down from the button.
    let ps = c(200.0, 168.0);
    let ps_edge = Pos2::new(ps.x, ps.y + 7.5 * u);
    let ps_y = p(0.0, 412.0).y;
    painter.add(egui::Shape::line(vec![ps_edge, Pos2::new(ps.x, ps_y - 12.0 * scale)], leader));
    dot(ps_edge);
    callout_row(&painter, ps.x, ps_y, 0, Pill::Text("PS"), &value(16), tint(16), scale);
}

fn main() -> eframe::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mut screenshot_path = None;
    while let Some(argument) = args.next() {
        if argument == "--screenshot" {
            let Some(path) = args.next() else {
                eprintln!("Expected a file name after --screenshot");
                std::process::exit(2);
            };
            screenshot_path = Some(PathBuf::from(path));
        }
    }

    let screenshot_mode = screenshot_path.is_some();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1050.0, 680.0]);
    if screenshot_mode {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::viewport::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "DualLink Controller Studio",
        options,
        Box::new(move |creation_context| {
            apply_style(&creation_context.egui_ctx);
            Ok(Box::new(ControllerApp::new(screenshot_path)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{ds4_touchpad_click_from_report, mapped_button, migrate_profile_bindings, translate_controls, trigger_value,
        selection_after_delete, should_auto_enable, unique_profile_name, Profile,
    };
    use vigem_client::{XButtons, XGamepad};

    #[test]
    fn ds4_usb_touchpad_click_is_read_from_its_own_hid_bit() {
        let mut report = [0u8; 64];
        report[0] = 0x01;
        report[7] = 0x02;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(true));

        report[7] = 0x00;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(false));
        assert_eq!(ds4_touchpad_click_from_report(&report[..7]), None);
        // the PS button (bit 0) must not count as a touchpad click
        report[7] = 0x01;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(false));
    }

    #[test]
    fn ds4_bluetooth_touchpad_click_is_read_from_report_0x11() {
        let mut report = [0u8; 78];
        report[0] = 0x11;
        report[9] = 0x02;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(true));
        report[9] = 0x00;
        assert_eq!(ds4_touchpad_click_from_report(&report), Some(false));
        assert_eq!(ds4_touchpad_click_from_report(&report[..9]), None);
        // empty reads (timeouts) and unrelated reports never invent a press
        assert_eq!(ds4_touchpad_click_from_report(&[]), None);
        assert_eq!(ds4_touchpad_click_from_report(&[0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]), None);
    }

    #[test]
    fn modern_face_buttons_map_to_xbox_layout() {
        assert_eq!(mapped_button("Xbox A"), XButtons::A);
        assert_eq!(mapped_button("Xbox B"), XButtons::B);
        assert_eq!(mapped_button("Xbox X"), XButtons::X);
        assert_eq!(mapped_button("Xbox Y"), XButtons::Y);
    }

    #[test]
    fn analog_triggers_map_independently_and_retain_pressure() {
        let mut pressed = [false; 18];
        pressed[10] = true;
        let bindings = super::default_bindings();
        let state = translate_controls(pressed, 0.42, 0.0, &bindings);
        assert_eq!(state.left_trigger, 107);
        assert_eq!(state.right_trigger, 0);

        let mut swapped = bindings;
        swapped[10] = "Right Trigger".into();
        let state = translate_controls(pressed, 0.42, 0.0, &swapped);
        assert_eq!(state.left_trigger, 0);
        assert_eq!(state.right_trigger, 107);
    }

    #[test]
    fn options_and_share_map_to_modern_menu_and_view_buttons() {
        let mut pressed = [false; 18];
        pressed[12] = true;
        pressed[13] = true;
        let state = translate_controls(pressed, 0.0, 0.0, &super::default_bindings());
        assert_eq!(
            state.buttons,
            XButtons::from(XButtons::START | XButtons::BACK)
        );
    }

    #[test]
    fn legacy_saved_layouts_migrate_to_the_eighteen_control_layout() {
        let legacy = vec![
            "D-pad Up",
            "D-pad Down",
            "D-pad Left",
            "D-pad Right",
            "Button 1",
            "Button 2",
            "Button 3",
            "Button 4",
            "Left Bumper",
            "Right Bumper",
            "Menu",
            "View",
            "Left Stick",
            "Right Stick",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let migrated = migrate_profile_bindings(legacy);
        assert_eq!(migrated.len(), 18);
        assert_eq!(migrated[4], "Xbox A");
        assert_eq!(migrated[10], "Left Trigger");
        assert_eq!(migrated[11], "Right Trigger");
        assert_eq!(migrated[12], "Xbox Menu");
        assert_eq!(migrated[13], "Xbox View");
    }

    #[test]
    fn analog_trigger_values_are_normalized() {
        assert_eq!(trigger_value(-1.0), 0.0);
        assert_eq!(trigger_value(0.5), 0.5);
        assert_eq!(trigger_value(2.0), 1.0);
    }

    #[test]
    fn unmapped_controls_produce_neutral_output() {
        let state = translate_controls([true; 18], 1.0, 1.0, &vec!["Unmapped".into(); 18]);
        assert_eq!(state, XGamepad::default());
    }

    #[test]
    fn new_profile_names_never_collide() {
        let profile = |name: &str| Profile { name: name.into(), bindings: Vec::new() };
        let profiles = vec![profile("Everyday"), profile("my profile 3")];
        assert_eq!(unique_profile_name(&profiles), "My profile 4");
        assert_eq!(unique_profile_name(&[]), "My profile 1");
    }

    #[test]
    fn selection_follows_the_right_profile_after_a_delete() {
        assert_eq!(selection_after_delete(2, 0, 3), 1);
        assert_eq!(selection_after_delete(0, 2, 3), 0);
        assert_eq!(selection_after_delete(1, 1, 3), 1);
        assert_eq!(selection_after_delete(2, 2, 2), 1);
    }

    #[test]
    fn xbox_output_starts_automatically_only_when_it_should() {
        // normal case: setting on, controller connected, nothing tried yet
        assert!(should_auto_enable(true, true, false, false, false));
        // setting off, no controller, already tried, switched off by the user, already on
        assert!(!should_auto_enable(false, true, false, false, false));
        assert!(!should_auto_enable(true, false, false, false, false));
        assert!(!should_auto_enable(true, true, true, false, false));
        assert!(!should_auto_enable(true, true, false, true, false));
        assert!(!should_auto_enable(true, true, false, false, true));
    }
}
