use eframe::egui::{self, CursorIcon};
use uuid::Uuid;

use super::connection_entry::{ConnectionEntry, ConnectionStatus};
use super::sidebar_action::SidebarAction;
use super::{context_menus, schema_renderer};
use crate::ui::icons;

/// Render all connections in the sidebar scroll area.
/// Returns an optional [`SidebarAction`] to be handled by the app.
pub(super) fn render_connections(
    ui: &mut egui::Ui,
    connections: &mut [ConnectionEntry],
) -> Option<SidebarAction> {
    let mut action: Option<SidebarAction> = None;
    let mut disconnect_id: Option<Uuid> = None;

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for entry in connections.iter_mut() {
                render_single_connection(ui, entry, &mut action, &mut disconnect_id);
            }
        });

    if let Some(id) = disconnect_id {
        action = Some(SidebarAction::Disconnect { conn_id: id });
    }
    action
}

fn render_single_connection(
    ui: &mut egui::Ui,
    entry: &mut ConnectionEntry,
    action: &mut Option<SidebarAction>,
    disconnect_id: &mut Option<Uuid>,
) {
    let conn_id = entry.conn_id;
    let label = truncate_label(&entry.label, 24);

    let header = match entry.status {
        ConnectionStatus::Connected => {
            build_header_label(&label, entry.all_databases.len(), &entry.visible_databases)
        }
        ConnectionStatus::Connecting => format!("{} \u{23F3}", label), // ⏳
        ConnectionStatus::Failed => format!("{} \u{26A0}", label),     // ⚠
        ConnectionStatus::Disconnected => label.clone(),
    };

    // For disconnected/failed/connecting — show header with arrow but no expandable body.
    if entry.status != ConnectionStatus::Connected {
        let header_id = ui.make_persistent_id(format!("conn:{conn_id}"));
        // Always force collapsed
        let mut cs = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            header_id,
            false,
        );
        cs.set_open(false);

        let text = match entry.status {
            ConnectionStatus::Failed => {
                egui::RichText::new(&header).color(egui::Color32::from_rgb(200, 80, 80))
            }
            ConnectionStatus::Connecting => egui::RichText::new(&header).weak(),
            _ => egui::RichText::new(&header),
        };

        let resp = cs
            .show_header(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(icons::engine::by_name(
                        &entry.driver_type.to_string(),
                        icons::SIDEBAR_ICON,
                    ));
                    ui.label(text)
                })
                .inner
                .on_hover_cursor(CursorIcon::PointingHand)
            })
            .body(|_| {}); // empty body — always collapsed

        let (toggle_resp, header_resp, _body) = resp;

        // Click on label or chevron → connect (if disconnected/failed)
        if (header_resp.inner.clicked() || toggle_resp.clicked())
            && (entry.status == ConnectionStatus::Disconnected
                || entry.status == ConnectionStatus::Failed)
        {
            // If chevron clicked, auto-expand after connection completes
            if toggle_resp.clicked() {
                entry.needs_expand = true;
            }
            *action = Some(SidebarAction::Connect { conn_id });
        }

        // Tooltip for failed
        if entry.status == ConnectionStatus::Failed {
            if let Some(err) = &entry.error_message {
                header_resp.inner.clone().on_hover_text(err.as_str());
            }
        }

        // Context menu on both toggle arrow and label
        context_menus::render_disconnected_context_menu(
            &header_resp.inner,
            conn_id,
            entry,
            action,
            disconnect_id,
        );
        context_menus::render_disconnected_context_menu(
            &toggle_resp,
            conn_id,
            entry,
            action,
            disconnect_id,
        );
        toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
        header_resp
            .response
            .on_hover_cursor(CursorIcon::PointingHand);
        return;
    }

    // ── Connected: full collapsing header with schema tree ──
    let header_id = ui.make_persistent_id(format!("conn:{conn_id}"));
    let default_open = !entry.needs_collapse;
    if entry.needs_collapse {
        entry.needs_collapse = false;
    }
    let mut cs = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        header_id,
        default_open,
    );
    // Auto-expand after connect triggered by chevron click
    if entry.needs_expand {
        cs.set_open(true);
        entry.needs_expand = false;
    }
    let resp = cs
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::engine::by_name(
                    &entry.driver_type.to_string(),
                    icons::SIDEBAR_ICON,
                ));
                ui.add(egui::Label::new(&header).sense(egui::Sense::click()))
            })
            .inner
            .on_hover_cursor(CursorIcon::PointingHand)
        })
        .body(|ui| {
            if let Some(schema) = &entry.schema {
                if let Some(a) = schema_renderer::render_schema_tree(
                    ui,
                    conn_id,
                    schema,
                    entry.visible_databases.as_ref(),
                    &mut entry.schema_detail_requested,
                    &mut entry.schemas_requested,
                ) {
                    *action = Some(a);
                }
            }
        });

    let (toggle_resp, header_resp, _body) = resp;

    // Click on label text also toggles expand/collapse.
    toggle_on_label_click(ui.ctx(), header_id, &header_resp.inner);

    context_menus::render_context_menu(&header_resp.inner, conn_id, entry, action, disconnect_id);
    context_menus::render_context_menu(&toggle_resp, conn_id, entry, action, disconnect_id);
    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .response
        .on_hover_cursor(CursorIcon::PointingHand);
    context_menus::render_database_picker(ui, conn_id, entry, action);
}

/// Toggle a `CollapsingState` when its label (header inner) is clicked.
/// Call this after `show_header(...).body(...)` with the `header_resp.response`
/// (the outer response, not the inner horizontal). The state is reloaded and
/// stored so the open/closed flag persists across frames.
pub(super) fn toggle_on_label_click(
    ctx: &eframe::egui::Context,
    header_id: eframe::egui::Id,
    label_resp: &eframe::egui::Response,
) {
    if label_resp.clicked() {
        if let Some(mut cs) =
            eframe::egui::collapsing_header::CollapsingState::load(ctx, header_id)
        {
            // Flip without needing a `ui` — avoids borrow conflicts in callers.
            let open = cs.is_open();
            cs.set_open(!open);
            cs.store(ctx);
        }
    }
}

pub(super) fn truncate_label(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", chars[..max_chars - 1].iter().collect::<String>())
    }
}

fn build_header_label(label: &str, total: usize, visible: &Option<Vec<String>>) -> String {
    let badge = match visible {
        Some(v) => format!("{}/{}", v.len(), total),
        None => total.to_string(),
    };
    format!("{}  [{}]", label, badge)
}
