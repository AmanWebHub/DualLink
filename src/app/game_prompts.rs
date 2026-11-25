use eframe::egui;
use std::path::{Path, PathBuf};

/// Initial game-specific prompt-pack manager. It deliberately installs a user-supplied
/// modified archive; DualLink does not redistribute game or mod assets.
pub(super) struct GamePromptManager {
    game_folder: String,
    mod_archive: String,
    status: String,
}

impl Default for GamePromptManager {
    fn default() -> Self {
        Self {
            game_folder: String::new(),
            mod_archive: String::new(),
            status: "Choose your Dying Light installation folder to begin.".into(),
        }
    }
}

impl GamePromptManager {
    fn target_archive(&self) -> PathBuf {
        Path::new(self.game_folder.trim()).join("DW").join("Data0.pak")
    }

    fn backup_archive(&self) -> PathBuf {
        let mut backup = self.target_archive();
        backup.set_file_name("Data0.pak.duallink-backup");
        backup
    }

    fn valid_game_folder(&self) -> bool {
        let folder = Path::new(self.game_folder.trim());
        folder.join("DW").join("Data0.pak").is_file()
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("Game prompts");
        ui.label("Manage PlayStation button prompts for supported games.");
        ui.add_space(16.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.heading("Dying Light (original)");
            ui.label("PC version · Xbox-compatible input with PlayStation button artwork");
            ui.add_space(8.0);
            ui.label("Game installation folder");
            ui.add(egui::TextEdit::singleline(&mut self.game_folder)
                .hint_text(r"Example: C:\Program Files (x86)\Steam\steamapps\common\Dying Light")
                .desired_width(f32::INFINITY));
            let valid = self.valid_game_folder();
            ui.horizontal(|ui| {
                ui.label(if valid { "✓ Data0.pak found in DW" } else { "Paste the folder that contains the DW folder" });
            });
            ui.add_space(8.0);
            ui.label("PlayStation prompt mod archive (modified Data0.pak)");
            ui.add(egui::TextEdit::singleline(&mut self.mod_archive)
                .hint_text("Paste the full path to the modded Data0.pak file")
                .desired_width(f32::INFINITY));
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(valid && Path::new(self.mod_archive.trim()).is_file(), egui::Button::new("Install prompt archive")).clicked() {
                    self.install();
                }
                if ui.add_enabled(valid && self.backup_archive().is_file(), egui::Button::new("Restore original")).clicked() {
                    self.restore();
                }
            });
            ui.add_space(8.0);
            ui.label(egui::RichText::new(format!("Backup: {}", self.backup_archive().display())).small());
        });

        ui.add_space(12.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Safety notes").strong());
            ui.label("DualLink creates a one-time backup before replacing the archive. It will not overwrite an existing backup.");
            ui.label("Use a prompt mod made for your Dying Light edition/version. This first version installs a complete modded Data0.pak; it does not yet merge individual textures or support every PAK layout.");
        });
        ui.add_space(8.0);
        ui.label(egui::RichText::new(&self.status).color(egui::Color32::from_rgb(145, 190, 255)));
    }

    fn install(&mut self) {
        let target = self.target_archive();
        let backup = self.backup_archive();
        let source = PathBuf::from(self.mod_archive.trim());
        if !source.is_file() {
            self.status = "The selected mod archive was not found.".into();
            return;
        }
        if source == target {
            self.status = "Select the modded archive separately; it cannot be the game's target file.".into();
            return;
        }
        let result = (|| -> Result<(), String> {
            if !backup.exists() {
                std::fs::copy(&target, &backup).map_err(|e| format!("Could not create original backup: {e}"))?;
            }
            std::fs::copy(&source, &target).map_err(|e| format!("Backup is saved, but install failed: {e}"))?;
            Ok(())
        })();
        self.status = match result {
            Ok(()) => format!("Installed prompt archive. Original backup is saved at {}", backup.display()),
            Err(error) => error,
        };
    }

    fn restore(&mut self) {
        let target = self.target_archive();
        let backup = self.backup_archive();
        self.status = match std::fs::copy(&backup, &target) {
            Ok(_) => "Original Data0.pak restored. The backup was kept for future use.".into(),
            Err(error) => format!("Could not restore the backup: {error}"),
        };
    }
}
