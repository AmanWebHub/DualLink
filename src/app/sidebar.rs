
use eframe::egui;
use egui::{Color32, Sense, Stroke, Vec2};

use super::{page_title, page_subtitle, ControllerApp};
use crate::theme::*;
use crate::widgets::*;

impl ControllerApp {
    pub(super) fn draw_sidebar(&mut self, ctx: &egui::Context) {
        // Load the sidebar logo once and keep its texture cached.
        let logo_id = egui::Id::new("duallink_sidebar_logo");

        let logo = ctx.data_mut(|data| {
            data.get_temp::<egui::TextureHandle>(logo_id)
        });

        let logo = logo.unwrap_or_else(|| {
            let image = image::load_from_memory(include_bytes!(
                "../../assets/DualLink-icon.png"
            ))
            .expect("Failed to load DualLink sidebar logo")
            .into_rgba8();

            let size = [image.width() as usize, image.height() as usize];

            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                size,
                image.as_raw(),
            );

            let texture = ctx.load_texture(
                "duallink_sidebar_logo",
                color_image,
                egui::TextureOptions::LINEAR,
            );

            ctx.data_mut(|data| {
                data.insert_temp(logo_id, texture.clone());
            });

            texture
        });

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
                        // Draw the DualLink logo instead of the old D icon.
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());

                        ui.painter().image(
                            logo.id(),
                            rect,
                            egui::Rect::from_min_max(
                                egui::Pos2::ZERO,
                                egui::Pos2::new(1.0, 1.0),
                            ),
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
                                        .color(if connected {
                                            GREEN
                                        } else {
                                            SIDEBAR_MUTED
                                        }),
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

                    ui.with_layout(
                        egui::Layout::bottom_up(egui::Align::Min),
                        |ui| {
                            ui.add_space(10.0);

                            ui.label(
                                egui::RichText::new(
                                    "A calm space to tune your\ncontroller layout.",
                                )
                                .size(11.0)
                                .color(SIDEBAR_MUTED),
                            );

                            ui.label(
                                egui::RichText::new("DUALLINK  ·  PERSONAL")
                                    .size(9.0)
                                    .strong()
                                    .color(SIDEBAR_MUTED),
                            );
                        },
                    );
                });
            });
    }

    pub(super) fn draw_header(&mut self, ui: &mut egui::Ui) {
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

            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add(primary_button("Save profile", 120.0))
                        .clicked()
                    {
                        self.save_profiles();
                    }

                    ui.add_space(12.0);

                    self.output_switch(ui);
                },
            );
        });
    }

    /// The always-visible Xbox output switch shown at the top of every page.
    pub(super) fn output_switch(&mut self, ui: &mut egui::Ui) {
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
                if on {
                    Color32::from_rgb(168, 224, 199)
                } else {
                    LINE
                },
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

                // Explicit direction: horizontal would flip inside
                // the header's right-to-left row.
                ui.with_layout(
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Xbox output")
                                    .size(12.5)
                                    .strong()
                                    .color(INK),
                            );

                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;

                                ui.label(
                                    egui::RichText::new("●")
                                        .size(9.0)
                                        .color(dot),
                                );

                                ui.label(
                                    egui::RichText::new(status)
                                        .size(10.5)
                                        .color(MUTED),
                                );

                                if failed {
                                    ui.hyperlink_to(
                                        egui::RichText::new("Get ViGEmBus")
                                            .size(10.5),
                                        DRIVER_URL,
                                    );
                                }
                            });
                        });

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let usable = on || connected;

                                if ui
                                    .add_enabled_ui(usable, |ui| {
                                        toggle_switch(ui, on)
                                    })
                                    .inner
                                    .clicked()
                                {
                                    toggle = true;
                                }
                            },
                        );
                    },
                );
            });

        if toggle {
            self.set_output(!on);
        }
    }
}
