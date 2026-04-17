//! SQL export options and UI rendering.
//!
//! Per-table toggles (Structure, Drop, Data) live on `ExportTableItem`.
//! Global options live here (batch size, gzip).

use eframe::egui;

// ── Options ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchSize {
    One,
    Hundred,
    FiveHundred,
    Thousand,
}

impl BatchSize {
    pub fn label(&self) -> &'static str {
        match self {
            BatchSize::One => "1 (one INSERT per row)",
            BatchSize::Hundred => "100",
            BatchSize::FiveHundred => "500",
            BatchSize::Thousand => "1000",
        }
    }
    pub fn value(&self) -> usize {
        match self {
            BatchSize::One => 1,
            BatchSize::Hundred => 100,
            BatchSize::FiveHundred => 500,
            BatchSize::Thousand => 1000,
        }
    }
    pub fn all() -> &'static [BatchSize] {
        &[
            BatchSize::One,
            BatchSize::Hundred,
            BatchSize::FiveHundred,
            BatchSize::Thousand,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct SqlOptions {
    pub batch_size: BatchSize,
    pub gzip: bool,
}

impl Default for SqlOptions {
    fn default() -> Self {
        Self {
            batch_size: BatchSize::FiveHundred,
            gzip: false,
        }
    }
}

// ── UI ──────────────────────────────────────────────────────────────────────

pub fn render_options_ui(ui: &mut egui::Ui, opts: &mut SqlOptions) {
    ui.label(
        egui::RichText::new(
            "Structure, Drop, and Data options are configured per table in the table list.",
        )
        .weak()
        .size(11.0),
    );
    ui.add_space(10.0);

    egui::Grid::new("sql_dropdowns")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("Rows per INSERT");
            egui::ComboBox::from_id_salt("sql_batch_size")
                .selected_text(opts.batch_size.label())
                .width(200.0)
                .show_ui(ui, |ui| {
                    for b in BatchSize::all() {
                        if ui
                            .selectable_label(opts.batch_size == *b, b.label())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            opts.batch_size = *b;
                        }
                    }
                });
            ui.end_row();
        });

    ui.add_space(4.0);
    ui.checkbox(&mut opts.gzip, "Compress the file using Gzip");
}
