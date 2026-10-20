use eframe::egui::{self, Color32, FontId, Stroke};

pub const BG: Color32 = Color32::from_rgb(248, 249, 251);
pub const PANEL: Color32 = Color32::from_rgb(255, 255, 255);
pub const CARD: Color32 = Color32::from_rgb(239, 242, 247);
pub const MUTED: Color32 = Color32::from_rgb(111, 121, 138);
pub const TEXT: Color32 = Color32::from_rgb(30, 39, 53);
pub const ACCENT: Color32 = Color32::from_rgb(57, 101, 169);

pub fn install(ctx: &egui::Context) {
    #[cfg(windows)]
    if let Some(windows) = std::env::var_os("WINDIR") {
        if let Ok(bytes) =
            std::fs::read(std::path::PathBuf::from(windows).join("Fonts/segoeui.ttf"))
        {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("system-ui".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "system-ui".into());
            ctx.set_fonts(fonts);
        }
    }
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BG;
    style.visuals.faint_bg_color = PANEL;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.selection.bg_fill = Color32::from_rgb(226, 235, 249);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, TEXT);
    style.visuals.window_corner_radius = 12.into();
    style.visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(223, 228, 236));
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = 6.into();
        widget.bg_stroke = Stroke::NONE;
    }
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(228, 234, 244);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(211, 225, 246);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.window_margin = 20.into();
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(24.0));
    ctx.set_style(style);
}

pub fn badge(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.0).color(MUTED));
        });
}
