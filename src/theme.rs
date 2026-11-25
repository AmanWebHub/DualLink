use eframe::egui;
use egui::{Color32, Stroke, Vec2};

pub(crate) const BG: Color32 = Color32::from_rgb(243, 246, 250);
pub(crate) const NAVY: Color32 = Color32::from_rgb(21, 33, 54);
pub(crate) const BLUE: Color32 = Color32::from_rgb(77, 111, 240);
pub(crate) const INK: Color32 = Color32::from_rgb(35, 48, 69);
pub(crate) const MUTED: Color32 = Color32::from_rgb(124, 137, 157);
pub(crate) const GREEN: Color32 = Color32::from_rgb(40, 166, 119);
pub(crate) const LINE: Color32 = Color32::from_rgb(231, 235, 241);
pub(crate) const RED: Color32 = Color32::from_rgb(214, 69, 69);
pub(crate) const SIDEBAR_MUTED: Color32 = Color32::from_rgb(143, 162, 193);
pub(crate) const DRIVER_URL: &str = "https://github.com/nefarius/ViGEmBus/releases";

/// App-wide look: soft rounded controls, blue accents, white popups.
pub(crate) fn apply_style(ctx: &egui::Context) {
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
