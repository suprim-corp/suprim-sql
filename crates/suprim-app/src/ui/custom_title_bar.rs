//! Custom title bar rendered by egui on macOS (fullsize-content-view mode).
//!
//! Replaces the native title bar with an egui panel that:
//! - Reserves left padding for the traffic-light buttons.
//! - Shows notification bell icon and hamburger menu on the right.
//! - Enables window dragging on the empty area.

use eframe::egui::{self, CursorIcon, RichText, Sense};

/// Height of the custom title bar in points.
/// macOS traffic lights are ~14pt tall, centered in a ~28pt title bar.
const TITLE_BAR_HEIGHT: f32 = 28.0;

/// Left padding to avoid overlapping macOS traffic-light buttons.
const TRAFFIC_LIGHT_PADDING: f32 = 70.0;

/// Render the custom title bar at the top of the window.
pub fn show_title_bar(ui: &mut egui::Ui, tier_name: &str) -> TitleBarAction {
    let mut action = TitleBarAction::None;

    // Reposition traffic lights to vertically center within our title bar.
    super::macos_menu::center_traffic_lights(TITLE_BAR_HEIGHT as f64);

    egui::Panel::top("custom_title_bar")
        .exact_size(TITLE_BAR_HEIGHT)
        .show_separator_line(true)
        .frame(egui::Frame::NONE.fill(ui.visuals().panel_fill))
        .show_inside(ui, |ui| {
            // Remove inner margin so the panel height is exact.
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.horizontal_centered(|ui| {
                // ── Left: traffic-light padding ──
                ui.add_space(TRAFFIC_LIGHT_PADDING);

                // ── Drag area fills the middle ──
                // Reserve space for right icons: badge(~90) + bell(28) + hamburger(28) + spacing
                let right_reserve = 160.0;
                let available = ui.available_width() - right_reserve;
                let (_drag_rect, drag_resp) = ui.allocate_exact_size(
                    egui::vec2(available.max(0.0), TITLE_BAR_HEIGHT),
                    Sense::click_and_drag(),
                );
                if drag_resp.dragged() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag_resp.double_clicked() {
                    let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                // ── Right: tier badge + notification bell + hamburger ──
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0); // right edge padding

                    // Hamburger menu
                    let hamburger = ui
                        .add(
                            egui::Button::new(
                                RichText::new(egui_phosphor::regular::LIST).size(20.0),
                            )
                            .frame(false),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand);

                    egui::Popup::menu(&hamburger).show(|ui| {
                        ui.set_min_width(180.0);
                        if ui
                            .button(format!("{}  About SuprimSQL", egui_phosphor::regular::INFO))
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            action = TitleBarAction::AboutClicked;
                            ui.close();
                        }
                    });

                    // Notification bell
                    let bell = ui
                        .add(
                            egui::Button::new(
                                RichText::new(egui_phosphor::regular::BELL).size(20.0),
                            )
                            .frame(false),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand);
                    if bell.clicked() {
                        action = TitleBarAction::NotificationClicked;
                    }

                    ui.add_space(4.0);

                    // ── Tier badge (painter-based for precise vertical centering) ──
                    let (icon, label, text_color, bg_color) = match tier_name {
                        "Premium" => (
                            egui_phosphor::regular::CROWN,
                            "Premium",
                            egui::Color32::from_rgb(100, 60, 0),
                            egui::Color32::from_rgb(255, 200, 80),
                        ),
                        _ => (
                            egui_phosphor::regular::LOCK_SIMPLE,
                            "Free",
                            ui.visuals().weak_text_color(),
                            if ui.visuals().dark_mode {
                                egui::Color32::from_rgba_premultiplied(255, 255, 255, 15)
                            } else {
                                egui::Color32::from_rgba_premultiplied(0, 0, 0, 12)
                            },
                        ),
                    };

                    let badge_text = format!("{icon} {label}");
                    let font_id = egui::FontId::proportional(10.0);
                    let galley = ui.painter().layout_no_wrap(badge_text, font_id, text_color);
                    let pad_h: f32 = 8.0; // horizontal padding
                    let pad_v: f32 = 2.0; // vertical padding
                    let badge_w = galley.size().x + pad_h * 2.0;
                    let badge_h = galley.size().y + pad_v * 2.0;

                    // Allocate space for the badge, let layout handle horizontal positioning
                    let (badge_rect, badge_resp) = ui
                        .allocate_exact_size(egui::vec2(badge_w, TITLE_BAR_HEIGHT), Sense::click());

                    // Center the pill vertically within the title bar height
                    let pill_rect = egui::Rect::from_center_size(
                        badge_rect.center(),
                        egui::vec2(badge_w, badge_h),
                    );

                    let painter = ui.painter();
                    painter.rect_filled(pill_rect, 4.0, bg_color);
                    painter.galley(
                        egui::pos2(pill_rect.left() + pad_h, pill_rect.top() + pad_v),
                        galley,
                        text_color,
                    );

                    if badge_resp.clicked() {
                        action = TitleBarAction::LicenseClicked;
                    }
                    let hover_text = if tier_name == "Premium" {
                        "Premium plan active"
                    } else {
                        "Free plan — click to manage license"
                    };
                    badge_resp
                        .on_hover_text(hover_text)
                        .on_hover_cursor(CursorIcon::PointingHand);
                });
            });
        });

    action
}

/// Actions that can be triggered from the custom title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBarAction {
    None,
    NotificationClicked,
    AboutClicked,
    LicenseClicked,
}
