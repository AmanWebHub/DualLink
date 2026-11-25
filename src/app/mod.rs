use eframe::egui;
use gilrs::{Axis, Button as PadButton, EventType, GamepadId, Gilrs};
use std::path::PathBuf;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::time::Duration;
use vigem_client::{Client, TargetId, XGamepad, Xbox360Wired};

use crate::hid::start_ds4_touchpad_reader;
use crate::mapping::*;
use crate::preview::PadView;
use crate::profiles::*;
use crate::settings::*;
use crate::theme::*;

mod pages;
mod sidebar;
mod game_prompts;

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

pub(crate) struct ControllerApp {
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
    game_prompts: game_prompts::GamePromptManager,
}

impl ControllerApp {
    pub(crate) fn new(screenshot_path: Option<PathBuf>) -> Self {
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
            game_prompts: game_prompts::GamePromptManager::default(),
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
                        "Game prompts" => self.game_prompts.show(ui),
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

fn page_title(page: &str) -> &str {
    match page {
        "Button mapping" => "Button mapping",
        "Input test" => "Input test",
        "Profiles" => "Your profiles",
        "Compatibility" => "PlayStation controller support",
        "How to use" => "How to use DualLink",
        "Game prompts" => "Game prompts",
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
        "Game prompts" => "Install and restore game-specific PlayStation button prompt mods.",
        _ => "A simple home for your controller layout.",
    }
}
