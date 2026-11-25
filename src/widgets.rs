use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::mapping::mapping_targets;
use crate::theme::*;

pub(crate) fn soft_shadow() -> egui::Shadow {
    egui::Shadow {
        offset: [0, 2],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(14),
    }
}

pub(crate) fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(egui::CornerRadius::same(16))
        .inner_margin(egui::Margin::same(18))
        .shadow(soft_shadow())
        .show(ui, content);
}

pub(crate) fn primary_button(label: &str, min_width: f32) -> egui::Button<'static> {
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

pub(crate) fn secondary_button(label: &str, min_width: f32) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(label).size(13.0).strong().color(BLUE))
        .fill(Color32::from_rgb(237, 241, 255))
        .stroke(Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(10))
        .min_size(Vec2::new(min_width, 38.0))
}

/// An on/off switch. Returns the click response; the caller decides what to do with it.
pub(crate) fn toggle_switch(ui: &mut egui::Ui, on: bool) -> egui::Response {
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
pub(crate) fn nav_item(ui: &mut egui::Ui, marker: &str, label: &str, selected: bool) -> egui::Response {
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

pub(crate) fn metric_card(ui: &mut egui::Ui, title: &str, value: &str, detail: &str, accent: Color32) {
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

pub(crate) fn section_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(title).size(15.0).strong().color(INK));
        ui.label(egui::RichText::new(subtitle).size(10.5).color(MUTED));
    });
}

pub(crate) fn mapping_row(ui: &mut egui::Ui, index: usize, action: &str, binding: Option<&mut String>) {
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

pub(crate) fn binding_row(ui: &mut egui::Ui, action: &str, value: &str, button: Option<&str>) -> bool {
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

pub(crate) fn pill(ui: &mut egui::Ui, label: &str, background: Color32, foreground: Color32) {
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

pub(crate) fn stick_controls(ui: &mut egui::Ui, label: &str, x: &mut f32, y: &mut f32) {
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
