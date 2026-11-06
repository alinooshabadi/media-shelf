use eframe::egui::{self, Color32, FontFamily, FontId, Stroke};

// Shared desktop geometry and semantic color tokens.
pub const RADIUS_CONTROL: u8 = 10;
pub const RADIUS_PANEL: u8 = 14;
pub const RADIUS_SURFACE: u8 = 18;
pub const CONTROL_SM: f32 = 30.0;
pub const CONTROL_MD: f32 = 36.0;
pub const CONTROL_LG: f32 = 44.0;
pub const GROUP: f32 = 16.0;
pub const REGION: f32 = 24.0;
pub const SECTION: f32 = 32.0;

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub muted_surface: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub selection: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
}

// Convert the upstream OKLab tokens to sRGB once when constructing a palette.
fn oklch(l: f32, c: f32, h: f32) -> Color32 {
    let (sin, cos) = h.to_radians().sin_cos();
    let (a, b) = (c * cos, c * sin);
    let ll = (l + 0.39633778 * a + 0.21580376 * b).powi(3);
    let m = (l - 0.105561346 * a - 0.06385417 * b).powi(3);
    let s = (l - 0.08948418 * a - 1.2914855 * b).powi(3);
    let channel = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let v = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (v * 255.0).round() as u8
    };
    Color32::from_rgb(
        channel(4.0767417 * ll - 3.3077116 * m + 0.23096994 * s),
        channel(-1.268438 * ll + 2.6097574 * m - 0.3413194 * s),
        channel(-0.0041960863 * ll - 0.7034186 * m + 1.7076147 * s),
    )
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        let gray = |light, dark_level| oklch(if dark { dark_level } else { light }, 0.0, 0.0);
        let accent = if dark {
            Color32::from_rgb(107, 163, 255)
        } else {
            Color32::from_rgb(5, 98, 239)
        };
        Self {
            background: gray(1.0, 0.18),
            surface: gray(1.0, 0.21),
            raised: gray(1.0, 0.24),
            muted_surface: gray(0.978, 0.26),
            text: gray(0.15, 0.96),
            secondary: gray(0.46, 0.77),
            muted: gray(0.565, 0.64),
            border: gray(0.915, 0.29),
            border_strong: gray(0.665, 0.515),
            accent,
            accent_text: if dark {
                Color32::from_rgb(11, 21, 38)
            } else {
                Color32::WHITE
            },
            selection: Color32::from_rgba_unmultiplied(5, 98, 239, if dark { 56 } else { 31 }),
            success: oklch(
                if dark { 0.79 } else { 0.50 },
                if dark { 0.14 } else { 0.12 },
                162.0,
            ),
            warning: oklch(
                if dark { 0.83 } else { 0.55 },
                0.12,
                if dark { 78.0 } else { 68.0 },
            ),
            danger: oklch(
                if dark { 0.70 } else { 0.55 },
                if dark { 0.16 } else { 0.19 },
                if dark { 24.0 } else { 27.0 },
            ),
        }
    }
}

pub fn palette(ui: &egui::Ui) -> Palette {
    Palette::new(ui.visuals().dark_mode)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        (
            "Inter",
            include_bytes!("../../assets/fonts/Inter-Regular.ttf").as_slice(),
        ),
        (
            "Inter medium",
            include_bytes!("../../assets/fonts/Inter-Medium.ttf").as_slice(),
        ),
        (
            "Geist",
            include_bytes!("../../assets/fonts/Geist-Medium.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
        fonts
            .families
            .insert(FontFamily::Name(name.into()), vec![name.into()]);
    }
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Inter".into());
    ctx.set_fonts(fonts);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let dark = theme == egui::Theme::Dark;
        let p = Palette::new(dark);
        let mut style = egui::Style {
            visuals: if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            },
            ..Default::default()
        };
        let v = &mut style.visuals;
        v.panel_fill = p.background;
        v.window_fill = p.raised;
        v.extreme_bg_color = p.surface;
        v.text_edit_bg_color = Some(p.surface);
        v.faint_bg_color = p.muted_surface;
        v.override_text_color = None;
        v.weak_text_color = Some(p.secondary);
        v.hyperlink_color = p.accent;
        v.selection.bg_fill = p.selection;
        v.selection.stroke = Stroke::new(1.5_f32, p.accent);
        v.warn_fg_color = p.warning;
        v.error_fg_color = p.danger;
        v.disabled_alpha = 0.45;
        v.window_corner_radius = RADIUS_SURFACE.into();
        v.menu_corner_radius = RADIUS_PANEL.into();
        v.window_stroke = Stroke::new(1.0_f32, p.border);
        v.window_shadow = egui::epaint::Shadow {
            offset: [0, if dark { 14 } else { 12 }],
            blur: if dark { 36 } else { 32 },
            spread: 0,
            color: Color32::from_black_alpha(if dark { 102 } else { 31 }),
        };
        v.popup_shadow = v.window_shadow;
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = RADIUS_CONTROL.into();
            w.bg_fill = p.surface;
            w.weak_bg_fill = Color32::TRANSPARENT;
            w.bg_stroke = Stroke::new(1.0_f32, p.border);
            w.fg_stroke = Stroke::new(1.0_f32, p.text);
            w.expansion = 0.0;
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
        v.widgets.hovered.bg_fill = p.surface.blend(p.text.gamma_multiply(0.05));
        v.widgets.hovered.weak_bg_fill = v.widgets.hovered.bg_fill;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
        v.widgets.active.bg_fill = p.surface.blend(p.text.gamma_multiply(0.075));
        v.widgets.active.weak_bg_fill = v.widgets.active.bg_fill;
        v.widgets.active.bg_stroke = Stroke::new(1.5_f32, p.accent);
        v.widgets.open.bg_fill = p.muted_surface;
        style.animation_time = 0.14;
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.interact_size = egui::vec2(CONTROL_MD, CONTROL_MD);
        style.spacing.window_margin = 24.into();
        style.spacing.menu_margin = 6.into();
        style.spacing.indent = 16.0;
        style.spacing.icon_width = 16.0;
        style.spacing.icon_width_inner = 10.0;
        style.spacing.icon_spacing = 8.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::new(13.0, FontFamily::Name("Inter medium".into())),
        );
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.0));
        style.text_styles.insert(
            egui::TextStyle::Heading,
            FontId::new(22.0, FontFamily::Name("Inter medium".into())),
        );
        ctx.set_style_of(theme, style);
    }
    ctx.set_theme(egui::ThemePreference::Light);
}

pub fn text_edit(ui: &mut egui::Ui, edit: egui::TextEdit<'_>) -> egui::Response {
    let p = palette(ui);
    let original = ui.style().clone();
    let widgets = &mut ui.style_mut().visuals.widgets;
    for w in [&mut widgets.inactive, &mut widgets.hovered] {
        w.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
    }
    let response = ui.add(
        edit.margin(egui::Margin::symmetric(12, 9))
            .min_size(egui::vec2(0.0, CONTROL_MD)),
    );
    ui.set_style(original);
    response
}

// Keep the widget in its parent's layout so wrapped checkbox groups still wrap.
pub fn checkbox(ui: &mut egui::Ui, checked: &mut bool, text: &str) -> egui::Response {
    let p = palette(ui);
    let original = ui.style().clone();
    ui.visuals_mut().override_text_color = Some(p.text);
    let widgets = &mut ui.visuals_mut().widgets;
    for w in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
    ] {
        w.corner_radius = 4.into();
        w.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
    }
    let response = ui.checkbox(checked, text);
    ui.set_style(original);
    response
}

pub fn spinner(ui: &mut egui::Ui) -> egui::Response {
    let reduced = ui.ctx().data_mut(|d| {
        d.get_temp::<bool>(egui::Id::new("reduced_motion"))
            .unwrap_or(false)
    });
    if reduced {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
        ui.painter()
            .circle_stroke(rect.center(), 6.0, Stroke::new(2.0_f32, palette(ui).accent));
        response
    } else {
        ui.add(egui::Spinner::new().size(16.0))
    }
}

pub fn primary(ui: &egui::Ui, text: &str) -> egui::Button<'static> {
    let p = palette(ui);
    egui::Button::new(egui::RichText::new(text).color(p.accent_text)).fill(p.accent)
}

pub fn badge(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(12.0)
            .color(palette(ui).muted),
    );
}

pub fn appearance(ui: &mut egui::Ui) {
    ui.heading("Appearance");
    let mut preference = ui.ctx().options(|o| o.theme_preference);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut preference, egui::ThemePreference::Light, "Light");
        ui.selectable_value(&mut preference, egui::ThemePreference::Dark, "Dark");
        ui.selectable_value(&mut preference, egui::ThemePreference::System, "System");
    });
    ui.ctx().set_theme(preference);
    let mut reduced = ui.ctx().data_mut(|d| {
        d.get_temp::<bool>(egui::Id::new("reduced_motion"))
            .unwrap_or(false)
    });
    if checkbox(ui, &mut reduced, "Reduce motion").changed() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("reduced_motion"), reduced));
        ui.ctx()
            .all_styles_mut(|s| s.animation_time = if reduced { 0.0 } else { 0.14 });
    }
    ui.add_space(GROUP);
    ui.separator();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reduced_motion_loading_keeps_its_size_and_static_feedback() {
        let ctx = egui::Context::default();
        install(&ctx);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("reduced_motion"), true));
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(spinner(ui).rect.size(), egui::vec2(16.0, 16.0));
            });
        });
        assert!(output.shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Circle(circle) if circle.radius == 6.0 && circle.stroke.width == 2.0)));
    }

    #[test]
    fn design_tokens_and_both_themes_are_installed() {
        let ctx = egui::Context::default();
        install(&ctx);
        assert_eq!(oklch(1.0, 0.0, 0.0), Color32::WHITE);
        for (theme, page, surface, edge) in [
            (egui::Theme::Light, 255, 255, 148),
            (egui::Theme::Dark, 18, 24, 103),
        ] {
            let style = ctx.style_of(theme);
            assert_eq!(style.visuals.panel_fill, Color32::from_gray(page));
            assert_eq!(style.visuals.extreme_bg_color, Color32::from_gray(surface));
            assert_eq!(
                Palette::new(theme == egui::Theme::Dark).border_strong,
                Color32::from_gray(edge)
            );
            assert_eq!(style.visuals.disabled_alpha, 0.45);
            assert_eq!(style.spacing.interact_size.y, CONTROL_MD);
            assert_eq!(style.visuals.widgets.hovered.expansion, 0.0);
        }
        assert_eq!(CONTROL_SM, 30.0);
        assert_eq!(CONTROL_LG, 44.0);
    }
}
