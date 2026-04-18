/// Per-column filter popup — anchored below the column header.
/// Renders operator dropdown, value input(s), and Apply/Clear buttons.
use eframe::egui;

use super::column_filter::{ColumnFilter, ColumnFilterState, FilterOperator};

/// State for the currently open filter popup.
pub struct FilterPopupState {
    /// Which column's popup is open. `None` = closed.
    pub open_column: Option<String>,
    /// Temp editing state (copied from ColumnFilterState on open, applied on Apply).
    pub editing: Option<ColumnFilter>,
    /// Screen position to anchor the popup below the column header.
    pub anchor_pos: egui::Pos2,
}

impl Default for FilterPopupState {
    fn default() -> Self {
        Self {
            open_column: None,
            editing: None,
            anchor_pos: egui::Pos2::ZERO,
        }
    }
}

impl FilterPopupState {
    /// Open popup for a column. Copies existing filter or creates a new one.
    pub fn open(
        &mut self,
        column: &str,
        db_type: &str,
        filters: &ColumnFilterState,
        anchor: egui::Pos2,
    ) {
        let filter = filters
            .get(column)
            .cloned()
            .unwrap_or_else(|| ColumnFilter::new(column, db_type));
        self.open_column = Some(column.to_string());
        self.editing = Some(filter);
        self.anchor_pos = anchor;
    }

    pub fn close(&mut self) {
        self.open_column = None;
        self.editing = None;
    }

    pub fn is_open(&self) -> bool {
        self.open_column.is_some()
    }
}

/// Outcome of rendering the popup.
pub enum FilterPopupOutcome {
    /// Still editing, no action needed.
    Pending,
    /// User clicked Apply — contains the filter to set.
    Apply(ColumnFilter),
    /// User clicked Clear — remove filter for this column.
    Clear(String),
}

/// Render the filter popup as an `egui::Area` anchored at the popup state's position.
/// Must be called OUTSIDE the TableBuilder (egui restriction).
pub fn render_filter_popup(
    ctx: &egui::Context,
    state: &mut FilterPopupState,
) -> FilterPopupOutcome {
    if !state.is_open() {
        return FilterPopupOutcome::Pending;
    }

    let editing = match &mut state.editing {
        Some(e) => e,
        None => return FilterPopupOutcome::Pending,
    };

    let mut outcome = FilterPopupOutcome::Pending;

    // Unique ID for the popup area
    let area_id = egui::Id::new("column_filter_popup");

    let area_resp = egui::Area::new(area_id)
        .order(egui::Order::Foreground)
        .fixed_pos(state.anchor_pos)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(210.0);
                ui.set_max_width(280.0);

                // Title
                let col_name = editing.column.clone();
                ui.label(
                    egui::RichText::new(format!("Filter: {col_name}"))
                        .strong()
                        .size(13.0),
                );
                ui.add_space(4.0);

                // Operator ComboBox
                let current_label = editing.operator.label();
                egui::ComboBox::from_id_salt("filter_op_combo")
                    .selected_text(current_label)
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for op in FilterOperator::all() {
                            let is_selected = editing.operator == *op;
                            if ui.selectable_label(is_selected, op.label()).clicked() {
                                editing.operator = *op;
                            }
                        }
                    });

                ui.add_space(4.0);

                // Value input — hidden for IS NULL / IS NOT NULL
                if editing.operator.needs_value() {
                    let hint = match editing.operator {
                        FilterOperator::In => "e.g. 1, 2, 3",
                        FilterOperator::Like | FilterOperator::NotLike => "e.g. %pattern%",
                        FilterOperator::Between => "From value",
                        _ => "Value",
                    };
                    ui.add(
                        egui::TextEdit::singleline(&mut editing.value)
                            .hint_text(hint)
                            .desired_width(170.0),
                    );

                    // Second value for BETWEEN
                    if editing.operator.needs_second_value() {
                        ui.add_space(2.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut editing.value2)
                                .hint_text("To value")
                                .desired_width(170.0),
                        );
                    }
                }

                ui.add_space(6.0);

                // Apply / Clear buttons
                ui.horizontal(|ui| {
                    let can_apply = !editing.operator.needs_value()
                        || !editing.value.is_empty()
                            && (!editing.operator.needs_second_value()
                                || !editing.value2.is_empty());

                    if ui
                        .add_enabled(can_apply, egui::Button::new("Apply"))
                        .clicked()
                    {
                        outcome = FilterPopupOutcome::Apply(editing.clone());
                    }

                    if ui.button("Clear").clicked() {
                        outcome = FilterPopupOutcome::Clear(col_name.clone());
                    }
                });
            });
        });

    // Close on Escape key
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if escape {
        state.close();
        return FilterPopupOutcome::Pending;
    }

    // Close if clicked outside the popup area
    let popup_rect = area_resp.response.rect;
    let clicked_outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && !popup_rect.contains(i.pointer.interact_pos().unwrap_or_default())
    });
    if clicked_outside {
        state.close();
        return FilterPopupOutcome::Pending;
    }

    outcome
}
