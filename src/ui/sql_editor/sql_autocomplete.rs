/// SQL editor autocomplete & auto-pair logic.
///
/// Two features:
/// 1. **Auto-pair** — typing `'`, `"`, `(`, `[`, `{` inserts the matching close char.
/// 2. **Keyword autocomplete** — popup with SQL keywords filtered by current word prefix.
///
/// All cursor positions from egui are **character offsets** (not byte offsets).
use eframe::egui;

use suprim_sql::db::sql_keywords::{SQL_CONSTANTS, SQL_FUNCTIONS, SQL_KEYWORDS, SQL_TYPES};

// ─── Char ↔ byte helpers ─────────────────────────────────────────────────────

/// Convert a **character index** to a **byte index** in a string.
/// Clamps to `text.len()` if char_idx is past the end.
fn char_to_byte(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

/// Convert a **byte index** to a **character index** in a string.
fn byte_to_char(text: &str, byte_idx: usize) -> usize {
    text[..byte_idx.min(text.len())].chars().count()
}

// ─── Autocomplete state ──────────────────────────────────────────────────────

/// Returned when the user accepts a suggestion from the popup.
pub struct AcceptedSuggestion {
    /// The replacement keyword (lowercase).
    pub replacement: String,
    /// Character offset of the prefix start that should be replaced.
    pub prefix_char_start: usize,
    /// Character length of the prefix to replace.
    pub prefix_char_len: usize,
}

/// Persistent state for the autocomplete popup, stored alongside the editor.
pub struct AutocompleteState {
    /// Whether the popup is open.
    pub open: bool,
    /// Filtered suggestions (owned strings — mix of column names + keywords).
    pub suggestions_owned: Vec<String>,
    /// Currently highlighted index in the suggestions list.
    pub selected_idx: usize,
    /// The word prefix that triggered the suggestions.
    pub prefix: String,
    /// **Character** offset of the prefix start in the text buffer.
    pub prefix_char_start: usize,
    /// Set when the user accepted a suggestion this frame (via Enter/Tab).
    accepted: bool,
    /// Set when the user dismissed the popup this frame (via Escape).
    dismissed: bool,
}

impl AutocompleteState {
    pub fn new() -> Self {
        Self {
            open: false,
            suggestions_owned: Vec::new(),
            selected_idx: 0,
            prefix: String::new(),
            prefix_char_start: 0,
            accepted: false,
            dismissed: false,
        }
    }

    /// Close the popup and reset state.
    pub fn close(&mut self) {
        self.open = false;
        self.suggestions_owned.clear();
        self.selected_idx = 0;
        self.prefix.clear();
        self.accepted = false;
        self.dismissed = false;
    }
}

// ─── Auto-pair ───────────────────────────────────────────────────────────────

/// **Phase 0** — Consume autocomplete navigation keys BEFORE TextEdit renders.
///
/// This prevents Enter/Tab from being processed by TextEdit (which would
/// insert a newline or lose focus). Must be called before `TextEdit::show()`.
pub fn consume_autocomplete_keys(ui: &mut egui::Ui, state: &mut AutocompleteState) {
    if !state.open || state.suggestions_owned.is_empty() {
        return;
    }

    ui.input_mut(|i| {
        if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
            state.dismissed = true;
        }
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
            state.selected_idx = (state.selected_idx + 1).min(state.suggestions_owned.len() - 1);
        }
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
            state.selected_idx = state.selected_idx.saturating_sub(1);
        }
        if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
            || i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
        {
            state.accepted = true;
        }
    });
}

// ─── Auto-pair bracket insertion ─────────────────────────────────────────────

/// Pair mapping: open -> close character.
const PAIRS: &[(char, char)] = &[('\'', '\''), ('"', '"'), ('(', ')'), ('[', ']'), ('{', '}')];

/// Check if a character was just typed and insert matching close bracket.
///
/// `cursor_char_pos` is the **character** position of the cursor (from `CCursor.index`).
/// Call **after** the TextEdit has been rendered (the character is already in the buffer).
/// Returns `true` if a pair was inserted.
pub fn handle_auto_pair(
    sql_text: &mut String,
    cursor_char_pos: Option<usize>,
    events: &[egui::Event],
) -> bool {
    // Find the last Text event this frame (the character just typed).
    let last_typed = events.iter().rev().find_map(|ev| {
        if let egui::Event::Text(t) = ev {
            Some(t.as_str())
        } else {
            None
        }
    });

    let typed_char = match last_typed {
        Some(s) if s.chars().count() == 1 => s.chars().next().unwrap(),
        _ => return false,
    };

    // Check if it's an open bracket.
    let close_char = match PAIRS.iter().find(|(o, _)| *o == typed_char) {
        Some((_, c)) => *c,
        None => return false,
    };

    let cursor_cpos = match cursor_char_pos {
        Some(p) => p,
        None => return false,
    };

    // Don't auto-pair quotes if the character before the typed one is alphanumeric
    // (e.g. typing apostrophe in "don't" shouldn't pair).
    if typed_char == '\'' || typed_char == '"' {
        if cursor_cpos >= 2 {
            // The char at cursor_cpos - 2 is the one before the just-typed char.
            let before_char = sql_text.chars().nth(cursor_cpos - 2);
            if let Some(ch) = before_char {
                if ch.is_alphanumeric() || ch == '_' {
                    return false;
                }
            }
        }
    }

    // Convert char position to byte position for insertion.
    let byte_pos = char_to_byte(sql_text, cursor_cpos);
    if byte_pos <= sql_text.len() {
        sql_text.insert(byte_pos, close_char);
        return true;
    }

    false
}

// ─── Keyword autocomplete ────────────────────────────────────────────────────

/// Extract the word being typed at the cursor position (the "prefix").
///
/// `cursor_char_pos` is a **character** offset.
/// Returns `(prefix_string, prefix_char_start)` or None if no word is being typed.
fn extract_word_at_cursor(text: &str, cursor_char_pos: usize) -> Option<(String, usize)> {
    if cursor_char_pos == 0 {
        return None;
    }

    let chars: Vec<char> = text.chars().collect();
    let cpos = cursor_char_pos.min(chars.len());

    // Walk backwards to find start of the current word (by char index).
    let mut word_char_start = cpos;
    while word_char_start > 0 {
        let ch = chars[word_char_start - 1];
        if ch.is_alphanumeric() || ch == '_' {
            word_char_start -= 1;
        } else {
            break;
        }
    }

    let prefix: String = chars[word_char_start..cpos].iter().collect();
    if prefix.len() < 2 {
        // Don't show suggestions for single-char prefixes — too noisy.
        return None;
    }

    Some((prefix, word_char_start))
}

/// Update autocomplete state based on current cursor position and text.
///
/// `cursor_char_pos` is a **character** offset (from `CCursor.index`).
/// `extra_words` provides additional suggestions (e.g. column names) — shown before keywords.
pub fn update_autocomplete(
    state: &mut AutocompleteState,
    text: &str,
    cursor_char_pos: usize,
    extra_words: &[String],
) {
    match extract_word_at_cursor(text, cursor_char_pos) {
        Some((prefix, char_start)) => {
            let upper = prefix.to_uppercase();
            let lower = prefix.to_lowercase();

            // Collect matching extra words (column names etc.) — case-insensitive.
            let mut results: Vec<String> = extra_words
                .iter()
                .filter(|w| {
                    let wl = w.to_lowercase();
                    wl.starts_with(&lower) && wl != lower
                })
                .cloned()
                .collect();

            // Append matching SQL keywords, types, functions, constants (lowercased for display).
            for set in [
                &*SQL_KEYWORDS,
                &*SQL_TYPES,
                &*SQL_FUNCTIONS,
                &*SQL_CONSTANTS,
            ] {
                for kw in set {
                    if kw.starts_with(&upper) && *kw != upper {
                        let lower_kw = kw.to_lowercase();
                        if !results.contains(&lower_kw) {
                            results.push(lower_kw);
                        }
                    }
                }
            }

            if results.is_empty() {
                state.close();
            } else {
                state.open = true;
                state.suggestions_owned = results;
                state.prefix = prefix;
                state.prefix_char_start = char_start;
                if state.selected_idx >= state.suggestions_owned.len() {
                    state.selected_idx = 0;
                }
            }
        }
        None => state.close(),
    }
}

/// Render the autocomplete popup and check for accepted suggestion.
///
/// `cursor_screen_pos` is the screen-space position of the text cursor,
/// used to anchor the popup right below the cursor.
///
/// Returns `AcceptedSuggestion` if the user confirmed a selection, else None.
/// Key consumption is done earlier by `consume_autocomplete_keys()`.
pub fn show_autocomplete_popup(
    ui: &mut egui::Ui,
    state: &mut AutocompleteState,
    text_edit_id: egui::Id,
    cursor_screen_pos: Option<egui::Pos2>,
) -> Option<AcceptedSuggestion> {
    if !state.open || state.suggestions_owned.is_empty() {
        return None;
    }

    if state.dismissed {
        state.close();
        return None;
    }

    if state.accepted {
        let keyword = state.suggestions_owned[state.selected_idx].clone();
        let result = AcceptedSuggestion {
            replacement: keyword,
            prefix_char_start: state.prefix_char_start,
            prefix_char_len: state.prefix.chars().count(),
        };
        state.close();
        return Some(result);
    }

    // Anchor popup below the text cursor, or fall back to editor top-left.
    let anchor = cursor_screen_pos.unwrap_or_else(|| ui.min_rect().left_top());

    // Compute min width from the longest suggestion.
    let longest = state
        .suggestions_owned
        .iter()
        .map(|kw| kw.len())
        .max()
        .unwrap_or(0);
    // Approximate: ~8px per char (monospace) + padding.
    let min_w = (longest as f32 * 8.0 + 24.0).max(60.0);

    let popup_id = text_edit_id.with("autocomplete");

    egui::Area::new(popup_id)
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(anchor.x, anchor.y))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(min_w);
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for (i, kw) in state.suggestions_owned.iter().enumerate() {
                            let is_selected = i == state.selected_idx;
                            let resp = ui.selectable_label(is_selected, kw);
                            if resp.clicked() {
                                state.selected_idx = i;
                            }
                            if is_selected {
                                resp.scroll_to_me(None);
                            }
                        }
                    });
            });
        });

    None
}

/// Apply the accepted suggestion by replacing the prefix in the text buffer.
///
/// `prefix_char_start` and `prefix_char_len` are **character** offsets.
/// Returns the new **character** cursor position.
pub fn apply_suggestion(
    sql_text: &mut String,
    prefix_char_start: usize,
    prefix_char_len: usize,
    replacement: &str,
) -> usize {
    let byte_start = char_to_byte(sql_text, prefix_char_start);
    let byte_end = char_to_byte(sql_text, prefix_char_start + prefix_char_len);

    if byte_end <= sql_text.len() {
        sql_text.replace_range(byte_start..byte_end, replacement);
        // Add a space after the keyword for convenience.
        let space_byte = byte_start + replacement.len();
        if space_byte <= sql_text.len() {
            sql_text.insert(space_byte, ' ');
        }
        // Return char position after the space.
        byte_to_char(sql_text, space_byte + 1)
    } else {
        byte_to_char(sql_text, sql_text.len())
    }
}
