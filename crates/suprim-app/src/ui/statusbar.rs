use eframe::egui;

use crate::update::state::SharedUpdateState;
use crate::update::UpdateState;

/// Bottom status bar.
pub struct StatusBar;

/// Action the status bar emits back to the app loop.
#[derive(Debug)]
pub enum StatusBarAction {
    /// User clicked the tier badge (Premium / Free) — opens license dialog.
    OpenLicense,
    /// User clicked the update badge in the "Available" state.
    InstallUpdate,
    /// User clicked the update badge in the "Failed" state.
    DismissUpdate,
}

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    /// Render the status bar. Update badge appears left of the tier badge
    /// when `update_state` is in a non-idle state.
    pub fn show(
        &self,
        ui: &mut egui::Ui,
        status: &str,
        tier_name: &str,
        update_state: &SharedUpdateState,
    ) -> Option<StatusBarAction> {
        let bar_h = ui.available_height();
        let mut action = None;

        ui.horizontal_centered(|ui| {
            ui.label(status);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Tier badge (Premium / Free) — always rightmost.
                if render_tier_badge(ui, bar_h, tier_name) {
                    action = Some(StatusBarAction::OpenLicense);
                }

                // Update badge — sits immediately to the left of the tier
                // badge when there's something worth surfacing.
                //
                // We `try_lock()` instead of blocking: if the async worker
                // holds the mutex during a slow network call, the UI would
                // otherwise beachball for the whole timeout. Skipping a
                // frame is invisible to the user (next repaint, ~16 ms
                // later, retries).
                let state_snapshot = update_state.try_lock().ok().map(|g| g.clone());
                if let Some(state) = state_snapshot {
                    if let Some(a) = render_update_badge(ui, bar_h, &state) {
                        action = Some(a);
                    }
                }
            });
        });

        action
    }
}

/// Returns `true` if the user clicked the tier badge.
fn render_tier_badge(ui: &mut egui::Ui, bar_h: f32, tier_name: &str) -> bool {
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

    let resp = draw_badge(
        ui,
        bar_h,
        &format!("{icon} {label}"),
        text_color,
        bg_color,
        true,
    );

    let tooltip = match tier_name {
        "Premium" => "Premium plan — click to manage license",
        _ => "Free plan — 5 connections max. Click to enter a license key.",
    };
    resp.clone().on_hover_text(tooltip);

    resp.clicked()
}

/// Visual + interaction config for the update badge.
struct UpdateBadgeStyle {
    icon: &'static str,
    label: String,
    text_color: egui::Color32,
    bg_color: egui::Color32,
    tooltip: Option<String>,
    click_action: Option<StatusBarAction>,
}

fn render_update_badge(
    ui: &mut egui::Ui,
    bar_h: f32,
    state: &UpdateState,
) -> Option<StatusBarAction> {
    let style = update_badge_style(ui, state)?;

    ui.add_space(6.0);
    let resp = draw_badge(
        ui,
        bar_h,
        &format!("{} {}", style.icon, style.label),
        style.text_color,
        style.bg_color,
        style.click_action.is_some(),
    );

    // Attach tooltip first; `on_hover_text` returns the same response so we
    // can still check `.clicked()` on it.
    let resp = match style.tooltip {
        Some(tip) => resp.on_hover_text(tip),
        None => resp,
    };

    if let Some(action) = style.click_action {
        if resp.clicked() {
            return Some(action);
        }
    }
    None
}

/// Returns the visual config for the update badge, or `None` when the badge
/// should be hidden entirely (Idle / Checking / UpToDate).
fn update_badge_style(ui: &egui::Ui, state: &UpdateState) -> Option<UpdateBadgeStyle> {
    let dark = ui.visuals().dark_mode;
    match state {
        UpdateState::Idle | UpdateState::Checking | UpdateState::UpToDate => None,

        UpdateState::Available(r) => Some(UpdateBadgeStyle {
            icon: egui_phosphor::regular::DOWNLOAD_SIMPLE,
            label: "Update available!".to_owned(),
            text_color: if dark {
                egui::Color32::from_rgb(200, 220, 255)
            } else {
                egui::Color32::from_rgb(20, 40, 90)
            },
            bg_color: if dark {
                egui::Color32::from_rgb(30, 60, 100)
            } else {
                egui::Color32::from_rgb(220, 235, 255)
            },
            tooltip: Some(format!(
                "SuprimSQL {} is available — click to install",
                r.version
            )),
            click_action: Some(StatusBarAction::InstallUpdate),
        }),

        UpdateState::Installing { progress, .. } => Some(UpdateBadgeStyle {
            icon: egui_phosphor::regular::ARROW_FAT_DOWN,
            label: progress.label(),
            text_color: ui.visuals().text_color(),
            bg_color: if dark {
                egui::Color32::from_rgb(40, 50, 60)
            } else {
                egui::Color32::from_rgb(230, 230, 230)
            },
            tooltip: Some("Installing update…".to_owned()),
            click_action: None,
        }),

        UpdateState::Relaunching => Some(UpdateBadgeStyle {
            icon: egui_phosphor::regular::CHECK_CIRCLE,
            label: "Relaunching…".to_owned(),
            text_color: if dark {
                egui::Color32::from_rgb(180, 240, 200)
            } else {
                egui::Color32::from_rgb(20, 80, 30)
            },
            bg_color: if dark {
                egui::Color32::from_rgb(30, 70, 40)
            } else {
                egui::Color32::from_rgb(220, 250, 220)
            },
            tooltip: None,
            click_action: None,
        }),

        UpdateState::Failed(msg) => Some(UpdateBadgeStyle {
            icon: egui_phosphor::regular::WARNING,
            label: "Update failed".to_owned(),
            text_color: if dark {
                egui::Color32::from_rgb(255, 200, 200)
            } else {
                egui::Color32::from_rgb(120, 20, 20)
            },
            bg_color: if dark {
                egui::Color32::from_rgb(90, 30, 30)
            } else {
                egui::Color32::from_rgb(255, 230, 230)
            },
            tooltip: Some(format!("{msg} — click to dismiss")),
            click_action: Some(StatusBarAction::DismissUpdate),
        }),
    }
}

/// Draw a pill-shaped badge with text. Returns the response so callers can
/// attach click handlers or hover tooltips.
///
/// `clickable` = `true` switches the cursor to pointing hand on hover and
/// lifts the background colour a notch so the user gets visual confirmation
/// the badge reacts to clicks. Tier badges are decorative (non-clickable)
/// so we leave them untouched.
fn draw_badge(
    ui: &mut egui::Ui,
    bar_h: f32,
    text: &str,
    text_color: egui::Color32,
    bg_color: egui::Color32,
    clickable: bool,
) -> egui::Response {
    let font_id = egui::FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font_id, text_color);
    let pad_h: f32 = 8.0;
    let pad_v: f32 = 2.0;
    let badge_w = galley.size().x + pad_h * 2.0;
    let badge_h = galley.size().y + pad_v * 2.0;

    let sense = if clickable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(badge_w, bar_h), sense);

    let center_y = rect.min.y + bar_h / 2.0;
    let pill = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, center_y),
        egui::vec2(badge_w, badge_h),
    );

    // Hover visual feedback for clickable badges: brighten the background
    // and darken it on active click.
    let effective_bg = if clickable && resp.is_pointer_button_down_on() {
        darken(bg_color, 0.85)
    } else if clickable && resp.hovered() {
        brighten(bg_color, 1.12)
    } else {
        bg_color
    };

    ui.painter().rect_filled(pill, 4.0, effective_bg);
    ui.painter().galley(
        egui::pos2(pill.left() + pad_h, pill.top() + pad_v),
        galley,
        text_color,
    );

    if clickable && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    resp
}

/// Multiply each RGB channel by `factor`, clamping to 255. `factor > 1.0`
/// lifts towards white; `factor < 1.0` pushes towards black. Alpha
/// is preserved.
fn brighten(c: egui::Color32, factor: f32) -> egui::Color32 {
    let clamp = |v: u8| ((v as f32 * factor).min(255.0)) as u8;
    egui::Color32::from_rgba_premultiplied(clamp(c.r()), clamp(c.g()), clamp(c.b()), c.a())
}

fn darken(c: egui::Color32, factor: f32) -> egui::Color32 {
    let clamp = |v: u8| ((v as f32 * factor).max(0.0)) as u8;
    egui::Color32::from_rgba_premultiplied(clamp(c.r()), clamp(c.g()), clamp(c.b()), c.a())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::{LatestRelease, UpdateProgress, UpdateState};
    use egui_kittest::Harness;
    use std::sync::{Arc, Mutex};

    fn sample_release(version: &str) -> LatestRelease {
        LatestRelease {
            version: version.to_owned(),
            channel: "stable".to_owned(),
            os: "macos".to_owned(),
            arch: "universal".to_owned(),
            download_url: "https://example.test/a.dmg".to_owned(),
            sha256: "a".repeat(64),
            size_bytes: 1,
            release_notes: None,
            release_url: None,
        }
    }

    /// Every state variant must render without panicking. Custom-painter
    /// badges bypass egui's accessibility tree, so we can't query labels —
    /// `run()` returning without error is the smoke test.
    #[test]
    fn renders_every_state_without_panic() {
        let release = sample_release("1.0.0");
        let variants = [
            UpdateState::Idle,
            UpdateState::Checking,
            UpdateState::UpToDate,
            UpdateState::Available(release.clone()),
            UpdateState::Installing {
                release: release.clone(),
                progress: UpdateProgress::Downloading {
                    bytes_done: 50,
                    bytes_total: 100,
                },
            },
            UpdateState::Installing {
                release: release.clone(),
                progress: UpdateProgress::Verifying,
            },
            UpdateState::Relaunching,
            UpdateState::Failed("boom".to_owned()),
        ];

        for v in variants {
            let state: SharedUpdateState = Arc::new(Mutex::new(v));
            let state_ref = state.clone();
            let mut harness = Harness::new_ui(move |ui| {
                ui.set_max_height(26.0);
                let _ = StatusBar::new().show(ui, "Ready", "Free", &state_ref);
            });
            harness.run();
        }
    }

    /// Pure-logic test: `update_badge_style` must return `None` for every
    /// state where the badge should not be shown.
    #[test]
    fn style_returns_none_for_invisible_states() {
        let harness = Harness::new_ui(|ui| {
            assert!(update_badge_style(ui, &UpdateState::Idle).is_none());
            assert!(update_badge_style(ui, &UpdateState::Checking).is_none());
            assert!(update_badge_style(ui, &UpdateState::UpToDate).is_none());
        });
        drop(harness);
    }

    #[test]
    fn style_returns_click_action_for_actionable_states() {
        let release = sample_release("2.0.0");
        let harness = Harness::new_ui(|ui| {
            let available = update_badge_style(ui, &UpdateState::Available(release.clone()))
                .expect("Available must render");
            assert!(matches!(
                available.click_action,
                Some(StatusBarAction::InstallUpdate)
            ));
            assert!(available.tooltip.as_deref().unwrap_or("").contains("2.0.0"));

            let failed = update_badge_style(ui, &UpdateState::Failed("err".to_owned()))
                .expect("Failed must render");
            assert!(matches!(
                failed.click_action,
                Some(StatusBarAction::DismissUpdate)
            ));
            assert!(failed.tooltip.as_deref().unwrap_or("").contains("err"));
        });
        drop(harness);
    }

    #[test]
    fn style_returns_no_click_action_for_in_progress_states() {
        let release = sample_release("1.0.0");
        let harness = Harness::new_ui(|ui| {
            let installing = update_badge_style(
                ui,
                &UpdateState::Installing {
                    release: release.clone(),
                    progress: UpdateProgress::Verifying,
                },
            )
            .expect("Installing must render");
            assert!(installing.click_action.is_none());

            let relaunching = update_badge_style(ui, &UpdateState::Relaunching)
                .expect("Relaunching must render");
            assert!(relaunching.click_action.is_none());
        });
        drop(harness);
    }
}
