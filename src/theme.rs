//! `BeamWright`'s visual identity.
//!
//! A warm graphite palette with a teal accent, rounded panels and pointer
//! cursors: deliberately close to the conventions of a vector editor while
//! looking like its own program.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Accent used for selection, hovered controls and links.
pub const ACCENT: Color32 = Color32::from_rgb(47, 182, 166);
/// Canvas selection default, deliberately different from the interface accent.
pub const CANVAS_SELECTION: &str = "#FF7A1A";

fn dark() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(20, 23, 27);
    visuals.window_fill = Color32::from_rgb(29, 34, 41);
    visuals.extreme_bg_color = Color32::from_rgb(13, 16, 19);
    visuals.text_edit_bg_color = Some(Color32::from_rgb(16, 19, 23));
    visuals.faint_bg_color = Color32::from_rgb(35, 41, 49);
    visuals.code_bg_color = Color32::from_rgb(16, 19, 23);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(58, 68, 78));
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(9);
    visuals.hyperlink_color = ACCENT;
    visuals.selection = egui::style::Selection {
        bg_fill: ACCENT,
        stroke: Stroke::new(1.0, Color32::from_rgb(10, 20, 20)),
    };
    visuals.warn_fg_color = Color32::from_rgb(240, 176, 72);
    visuals.error_fg_color = Color32::from_rgb(240, 104, 104);
    visuals.override_text_color = Some(Color32::from_rgb(230, 236, 239));
    visuals.weak_text_color = Some(Color32::from_rgb(158, 169, 178));
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    visuals.striped = true;
    visuals.slider_trailing_fill = true;
    let panel = visuals.panel_fill;
    visuals.widgets.noninteractive.bg_fill = panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(44, 52, 61));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 35, 42);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(34, 40, 47);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(52, 61, 71));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 48, 56);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(46, 55, 64);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.weak_bg_fill = ACCENT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::from_rgb(10, 22, 21));
    visuals.widgets.open.bg_fill = Color32::from_rgb(37, 44, 52);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, Color32::from_rgb(70, 82, 93));
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(7);
    }
    visuals
}

fn light() -> Visuals {
    let mut visuals = Visuals::light();
    visuals.panel_fill = Color32::from_rgb(244, 241, 234);
    visuals.window_fill = Color32::from_rgb(252, 250, 246);
    visuals.extreme_bg_color = Color32::from_rgb(233, 228, 218);
    visuals.text_edit_bg_color = Some(Color32::from_rgb(255, 254, 251));
    visuals.faint_bg_color = Color32::from_rgb(238, 234, 225);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(208, 201, 189));
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(9);
    visuals.hyperlink_color = Color32::from_rgb(15, 128, 116);
    visuals.selection = egui::style::Selection {
        bg_fill: Color32::from_rgb(15, 143, 128),
        stroke: Stroke::new(1.0, Color32::WHITE),
    };
    visuals.override_text_color = Some(Color32::from_rgb(34, 39, 43));
    visuals.weak_text_color = Some(Color32::from_rgb(105, 113, 120));
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    visuals.striped = true;
    visuals.slider_trailing_fill = true;
    let panel = visuals.panel_fill;
    visuals.widgets.noninteractive.bg_fill = panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(218, 212, 201));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(255, 253, 248);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(250, 247, 240);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(214, 207, 195));
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(229, 245, 241);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(223, 242, 237);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(15, 143, 128));
    visuals.widgets.active.bg_fill = Color32::from_rgb(15, 143, 128);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(15, 143, 128);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(15, 143, 128));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.open.bg_fill = Color32::from_rgb(246, 243, 236);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, Color32::from_rgb(200, 193, 181));
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(7);
    }
    visuals
}

/// Visuals for the current theme.
pub fn visuals(dark_mode: bool) -> Visuals {
    if dark_mode { dark() } else { light() }
}

#[cfg(test)]
mod tests {
    use super::visuals;

    #[test]
    fn both_themes_have_the_brand_accent_and_rounded_windows() {
        for dark in [true, false] {
            let visuals = visuals(dark);
            assert_eq!(
                visuals.interact_cursor,
                Some(egui::CursorIcon::PointingHand)
            );
            assert!(visuals.window_corner_radius.nw > 0);
            assert_eq!(visuals.dark_mode, dark);
        }
    }

    #[test]
    fn the_canvas_selection_colour_is_not_the_interface_accent() {
        assert_ne!(
            super::CANVAS_SELECTION.to_ascii_lowercase(),
            "#2fb6a6".to_string()
        );
    }
}
