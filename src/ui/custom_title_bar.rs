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
pub fn show_title_bar(ui: &mut egui::Ui) -> TitleBarAction {
    let mut action = TitleBarAction::None;

    egui::Panel::top("custom_title_bar")
        .exact_size(TITLE_BAR_HEIGHT)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(ui.visuals().panel_fill))
        .show_inside(ui, |ui| {
            // Remove inner margin so the panel height is exact.
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.horizontal_centered(|ui| {
                // ── Left: traffic-light padding ──
                ui.add_space(TRAFFIC_LIGHT_PADDING);

                // ── Drag area fills the middle ──
                let available = ui.available_width() - 56.0; // reserve for 2 icon buttons
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

                // ── Right: notification bell + hamburger ──
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0); // right edge padding
                    let hamburger = ui
                        .add(
                            egui::Button::new(
                                RichText::new(egui_phosphor::regular::LIST).size(20.0),
                            )
                            .frame(false),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand);

                    // Popup menu below hamburger button
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
}
