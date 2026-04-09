//! Diff results rendering — single-column tree list with operation badges.
//!
//! Each entry: [checkbox] [OP_BADGE] [object_icon] name (detail)
//! Groups are collapsible. Children indented under parent.

use eframe::egui;

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffGroup, DiffKind};

// ── Loading spinner ─────────────────────────────────────────────────────────

pub(crate) fn render_loading_state(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() / 3.0);
        ui.spinner();
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Comparing schemas...").weak());
    });
}

// ── Diff results ────────────────────────────────────────────────────────────

pub(crate) fn render_diff_results(ui: &mut egui::Ui, groups: &mut [DiffGroup]) {
    let total: usize = groups.iter().map(|g| g.total_count()).sum();

    if total == 0 {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.label(
                egui::RichText::new(format!(
                    "{}  Schemas are identical",
                    egui_phosphor::regular::CHECK_CIRCLE
                ))
                .size(14.0)
                .color(egui::Color32::from_rgb(76, 175, 80)),
            );
        });
        return;
    }

    // Fill all available height from parent allocate_ui
    let height = ui.available_height();

    egui::ScrollArea::vertical()
        .id_salt("diff_results")
        .min_scrolled_height(height)
        .max_height(height)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            for group in groups.iter_mut() {
                if group.entries.is_empty() {
                    continue;
                }
                render_group(ui, group);
            }
        });
}

// ── Group rendering ─────────────────────────────────────────────────────────

fn render_group(ui: &mut egui::Ui, group: &mut DiffGroup) {
    let style = kind_style(group.kind);

    let group_id = ui.make_persistent_id(format!("diff_group_{:?}", group.kind as u8));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), group_id, true);

    state
        .show_header(ui, |ui| {
            op_badge(ui, group.kind);
            ui.label(
                egui::RichText::new(format!(
                    "{} ({} of {} selected)",
                    group.label(),
                    group.checked_count(),
                    group.total_count()
                ))
                .strong()
                .color(style.color),
            );
        })
        .body_unindented(|ui| {
            for entry in group.entries.iter_mut() {
                render_entry(ui, entry, 1);
            }
        });
}

// ── Entry rendering ─────────────────────────────────────────────────────────

fn render_entry(ui: &mut egui::Ui, entry: &mut DiffEntry, depth: usize) {
    let has_children = !entry.children.is_empty();
    let indent = depth as f32 * 20.0;
    let style = kind_style(entry.kind);
    let obj_icon = entry.object_type.icon();

    // Build display text: "name (detail) — parent_table"
    let mut name_text = entry.name.clone();
    if !entry.detail.is_empty() {
        name_text.push_str(&format!("  ({})", entry.detail));
    }
    let table_suffix = entry
        .parent_table
        .as_ref()
        .map(|t| format!(" — {t}"))
        .unwrap_or_default();

    if has_children {
        // Collapsible parent entry
        let entry_id = ui.make_persistent_id(format!(
            "diff_entry_{}_{}",
            entry.object_type as u8, entry.name
        ));
        let cs = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            entry_id,
            false,
        );

        cs.show_header(ui, |ui| {
            ui.add_space(indent);
            ui.checkbox(&mut entry.checked, "");
            op_badge(ui, entry.kind);
            ui.label(egui::RichText::new(obj_icon).color(style.color).size(13.0));
            ui.label(&name_text);
            if !table_suffix.is_empty() {
                ui.label(egui::RichText::new(&table_suffix).weak().size(11.0));
            }
        })
        .body_unindented(|ui| {
            for child in entry.children.iter_mut() {
                render_entry(ui, child, depth + 1);
            }
        });
    } else {
        // Leaf entry — simple row
        ui.horizontal(|ui| {
            ui.add_space(indent + 20.0); // extra 20 to align with collapsible arrow
            ui.checkbox(&mut entry.checked, "");
            op_badge(ui, entry.kind);
            ui.label(egui::RichText::new(obj_icon).color(style.color).size(12.0));
            ui.label(egui::RichText::new(&name_text).size(12.0));
            if !table_suffix.is_empty() {
                ui.label(egui::RichText::new(&table_suffix).weak().size(11.0));
            }
        });
    }
}

// ── Operation badge ─────────────────────────────────────────────────────────

/// Renders a small colored badge like [CREATE] [MODIFY] [DELETE]
fn op_badge(ui: &mut egui::Ui, kind: DiffKind) {
    let style = kind_style(kind);
    let badge_w = style.badge_text.len() as f32 * 6.5 + 10.0;

    let (rect, _) = ui.allocate_exact_size(egui::vec2(badge_w, 16.0), egui::Sense::hover());
    let painter = ui.painter();

    painter.rect_filled(rect, 3.0, style.color.gamma_multiply(0.15));

    let font_id = egui::FontId::proportional(9.0);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        style.badge_text,
        font_id,
        style.color,
    );
}

// ── Styles ──────────────────────────────────────────────────────────────────

struct KindStyle {
    color: egui::Color32,
    badge_text: &'static str,
}

fn kind_style(kind: DiffKind) -> KindStyle {
    match kind {
        DiffKind::Modified => KindStyle {
            color: egui::Color32::from_rgb(33, 150, 243),
            badge_text: "MODIFY",
        },
        DiffKind::Added => KindStyle {
            color: egui::Color32::from_rgb(76, 175, 80),
            badge_text: "CREATE",
        },
        DiffKind::Removed => KindStyle {
            color: egui::Color32::from_rgb(244, 67, 54),
            badge_text: "DROP",
        },
    }
}
