/// Tab bar rendering — draws the horizontal strip of tab buttons with close icons.
/// Extracted from tab_manager.rs to separate rendering from tab lifecycle management.
use eframe::egui;
use uuid::Uuid;

/// Outcome of rendering the tab bar for a single frame.
pub struct TabBarOutput {
    /// Tab the user clicked to activate.
    pub activated: Option<Uuid>,
    /// Tab the user clicked the close button on.
    pub closed: Option<Uuid>,
}

/// Render the tab bar given a list of `(tab_id, label)` pairs and the current
/// active tab id. Returns which tab was activated / closed this frame.
pub fn render_tab_bar(
    ui: &mut egui::Ui,
    tabs: &[(Uuid, String)],
    active_tab: Option<Uuid>,
) -> TabBarOutput {
    let vis = ui.visuals().clone();
    let bar_bg = vis.faint_bg_color;
    let active_bg = vis.widgets.active.bg_fill;
    let hover_bg = vis.widgets.hovered.bg_fill;
    let active_border = vis.widgets.active.bg_stroke.color;
    let hover_border = vis.widgets.hovered.bg_stroke.color;
    let inactive_border = vis.widgets.noninteractive.bg_stroke.color;
    let active_text = vis.strong_text_color();
    let inactive_text = vis.widgets.inactive.fg_stroke.color;
    let close_active_color = vis.widgets.inactive.fg_stroke.color;
    let close_inactive_color = vis.weak_text_color();

    let mut output = TabBarOutput {
        activated: None,
        closed: None,
    };

    egui::Frame::NONE
        .fill(bar_bg)
        .inner_margin(egui::Margin::symmetric(2, 0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (tab_id, label_text) in tabs {
                    let is_active = active_tab == Some(*tab_id);

                    // Check hover from previous frame to adjust visuals
                    let tab_hover_id = egui::Id::new(("tab_hover", *tab_id));
                    let was_hovered = ui
                        .ctx()
                        .data(|d| d.get_temp::<bool>(tab_hover_id).unwrap_or(false));

                    let actual_bg = if is_active {
                        active_bg
                    } else if was_hovered {
                        hover_bg
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let actual_border = if is_active {
                        active_border
                    } else if was_hovered {
                        hover_border
                    } else {
                        inactive_border
                    };

                    let frame_response = egui::Frame::NONE
                        .fill(actual_bg)
                        .stroke(egui::Stroke::new(1.0, actual_border))
                        .inner_margin(egui::Margin::symmetric(6, 3))
                        .corner_radius(egui::CornerRadius::same(0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let text_color = if is_active {
                                    active_text
                                } else {
                                    inactive_text
                                };
                                let response = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(label_text).color(text_color),
                                    )
                                    .selectable(false)
                                    .sense(egui::Sense::click()),
                                );
                                if response.clicked() {
                                    output.activated = Some(*tab_id);
                                }

                                ui.add_space(6.0);
                                let close_color = if is_active {
                                    close_active_color
                                } else {
                                    close_inactive_color
                                };
                                let close_response = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(egui_phosphor::regular::X)
                                            .color(close_color),
                                    )
                                    .selectable(false)
                                    .sense(egui::Sense::click()),
                                );
                                if close_response.clicked() {
                                    output.closed = Some(*tab_id);
                                }
                            });
                        });

                    // Store hover state for next frame + set cursor
                    let tab_rect = frame_response.response.rect;
                    let is_hovered = ui.rect_contains_pointer(tab_rect);
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(tab_hover_id, is_hovered));
                    if is_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });
        });

    output
}
