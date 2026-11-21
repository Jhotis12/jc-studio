//! Small Markdown renderer for agent answers.
//!
//! Deliberately local: the app is pinned to egui 0.31 and the commonmark widgets move with
//! egui. This covers the constructs models actually emit — headings, paragraphs, lists,
//! fenced code, inline code, bold/italic and links — without a parser dependency.

use egui::{Color32, FontId, RichText, TextFormat};

pub fn render(ui: &mut egui::Ui, text: &str) {
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();
        let leading = trimmed.trim_start();

        if leading.starts_with("```") {
            let language = leading.trim_start_matches('`').trim().to_string();
            let mut code = String::new();
            for next in lines.by_ref() {
                if next.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(next);
                code.push('\n');
            }
            code_block(ui, &language, code.trim_end());
            continue;
        }

        if let Some((level, title)) = heading(leading) {
            let size = match level {
                1 => 21.0,
                2 => 18.5,
                3 => 16.5,
                _ => 15.0,
            };
            ui.add_space(if level <= 2 { 6.0 } else { 3.0 });
            ui.label(RichText::new(title).strong().size(size));
            continue;
        }

        if is_rule(leading) {
            ui.add_space(3.0);
            ui.separator();
            continue;
        }

        if leading.starts_with('>') {
            let quote = leading.trim_start_matches('>').trim();
            egui::Frame::new()
                .fill(Color32::from_rgb(24, 27, 33))
                .inner_margin(egui::Margin::symmetric(8, 2))
                .show(ui, |ui| {
                    ui.label(RichText::new(quote).italics().weak());
                });
            continue;
        }

        if let Some((indent, marker, content)) = list_item(trimmed) {
            ui.horizontal(|ui| {
                ui.add_space(indent as f32 * 14.0);
                ui.label(RichText::new(marker).weak());
                inline(ui, content);
            });
            continue;
        }

        if leading.is_empty() {
            ui.add_space(4.0);
            continue;
        }

        // A table is a run of lines containing pipes. The second line is the `---`
        // separator; render the rest as a grid.
        if leading.contains('|') {
            let mut rows = vec![trimmed.to_string()];
            while let Some(next) = lines.peek() {
                if next.trim().is_empty() || !next.contains('|') {
                    break;
                }
                rows.push(lines.next().unwrap_or_default().trim_end().to_string());
            }
            if rows.len() >= 2 && is_table_separator(&rows[1]) {
                rows.remove(1);
                table(ui, &rows);
            } else {
                for row in &rows {
                    inline(ui, row);
                }
            }
            continue;
        }

        // Paragraph: join continuation lines until a blank or a block construct.
        let mut paragraph = leading.to_string();
        while let Some(next) = lines.peek() {
            let next_leading = next.trim_start();
            if next_trimmed_empty(next)
                || next_leading.starts_with("```")
                || next_leading.starts_with('#')
                || next_leading.starts_with('>')
                || is_rule(next_leading)
                || list_item(next.trim_end()).is_some()
                || next_leading.contains('|')
            {
                break;
            }
            if let Some(next) = lines.next() {
                paragraph.push(' ');
                paragraph.push_str(next.trim());
            }
        }
        inline(ui, &paragraph);
    }
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = line[hashes..].strip_prefix(' ')?;
    Some((hashes, rest.trim()))
}

fn is_rule(line: &str) -> bool {
    let compact: String = line
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact.len() >= 3
        && (compact.chars().all(|character| character == '-')
            || compact.chars().all(|character| character == '*')
            || compact.chars().all(|character| character == '_'))
}

/// Returns `(indent, marker, content)` for bullet and numbered list items.
fn list_item(line: &str) -> Option<(usize, &'static str, &str)> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    for marker in ["- ", "* ", "+ "] {
        if let Some(content) = trimmed.strip_prefix(marker) {
            return Some((indent / 2, "•", content));
        }
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &trimmed[digits..];
        if let Some(content) = rest.strip_prefix(". ") {
            return Some((indent / 2, "‣", content));
        }
    }
    None
}

fn is_table_separator(line: &str) -> bool {
    line.chars()
        .all(|character| matches!(character, '-' | '|' | ':' | ' ') && !character.is_alphanumeric())
        && line.contains('-')
}

fn next_trimmed_empty(line: &str) -> bool {
    line.trim().is_empty()
}

fn table(ui: &mut egui::Ui, rows: &[String]) {
    if rows.is_empty() {
        return;
    }
    let cells = |line: &str| -> Vec<String> {
        line.trim()
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect()
    };
    let columns = rows.iter().map(|row| cells(row).len()).max().unwrap_or(1);
    egui::Grid::new(ui.id().with(("md-table", rows.first().cloned())))
        .num_columns(columns)
        .striped(true)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for (index, row) in rows.iter().enumerate() {
                for cell in cells(row) {
                    if index == 0 {
                        ui.label(RichText::new(cell).strong());
                    } else {
                        inline(ui, &cell);
                    }
                }
                ui.end_row();
            }
        });
}

fn code_block(ui: &mut egui::Ui, language: &str, code: &str) {
    if !language.is_empty() {
        ui.label(RichText::new(language).small().weak().monospace());
    }
    egui::Frame::new()
        .fill(Color32::from_rgb(15, 17, 22))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.label(RichText::new(code).monospace().size(13.0));
                });
        });
    ui.add_space(3.0);
}

/// Parses the common inline constructs and lays them out in one wrapping label.
fn inline(ui: &mut egui::Ui, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = ui.available_width().max(80.0);
    job.wrap.break_anywhere = false;

    let base = TextFormat {
        font_id: FontId::proportional(14.5),
        color: ui.visuals().text_color(),
        ..Default::default()
    };
    let strong = TextFormat {
        font_id: FontId::proportional(14.5),
        color: ui.visuals().strong_text_color(),
        extra_letter_spacing: 0.2,
        ..Default::default()
    };
    let italic = TextFormat {
        italics: true,
        ..base.clone()
    };
    let code = TextFormat {
        font_id: FontId::monospace(13.5),
        color: Color32::from_rgb(210, 190, 140),
        background: Color32::from_rgb(30, 33, 40),
        ..Default::default()
    };
    let link = TextFormat {
        color: Color32::from_rgb(120, 180, 245),
        underline: egui::Stroke::new(1.0_f32, Color32::from_rgb(80, 130, 200)),
        ..base.clone()
    };

    let characters: Vec<char> = text.chars().collect();
    let mut index = 0;
    let mut bold = false;
    let mut emphasis = false;
    while index < characters.len() {
        if characters[index] == '`' {
            let start = index + 1;
            if let Some(end) = characters[start..]
                .iter()
                .position(|character| *character == '`')
            {
                let value: String = characters[start..start + end].iter().collect();
                job.append(&value, 0.0, code.clone());
                index = start + end + 1;
                continue;
            }
            job.append("`", 0.0, base.clone());
            index += 1;
            continue;
        }
        if index + 1 < characters.len() && characters[index] == '*' && characters[index + 1] == '*'
        {
            if has_closing(&characters, index + 2, "**") {
                bold = !bold;
                index += 2;
                continue;
            }
            // A single asterisk, e.g. a glob such as `*.rs`.
            if has_closing(&characters, index + 1, "*") {
                emphasis = !emphasis;
                index += 1;
                continue;
            }
            job.append("*", 0.0, base.clone());
            index += 1;
            continue;
        }
        if characters[index] == '*' {
            if has_closing(&characters, index + 1, "*") {
                emphasis = !emphasis;
                index += 1;
                continue;
            }
            job.append("*", 0.0, base.clone());
            index += 1;
            continue;
        }
        if characters[index] == '_' {
            let prev_ok = index == 0 || !characters[index - 1].is_alphanumeric();
            let next_ok = index + 1 < characters.len() && !characters[index + 1].is_alphanumeric();
            if prev_ok && next_ok && has_closing(&characters, index + 1, "_") {
                emphasis = !emphasis;
                index += 1;
                continue;
            }
            job.append("_", 0.0, base.clone());
            index += 1;
            continue;
        }
        if characters[index] == '[' {
            if let Some((label, url, consumed)) = parse_link(&characters[index..]) {
                job.append(&format!("{label} ({url})"), 0.0, link.clone());
                index += consumed;
                continue;
            }
            job.append("[", 0.0, base.clone());
            index += 1;
            continue;
        }

        let start = index;
        while index < characters.len()
            && characters[index] != '`'
            && characters[index] != '*'
            && characters[index] != '_'
            && characters[index] != '['
        {
            index += 1;
        }
        let segment: String = characters[start..index].iter().collect();
        if segment.is_empty() {
            // Safety net: never spin on a character the branches above missed.
            job.append(&characters[index].to_string(), 0.0, base.clone());
            index += 1;
            continue;
        }
        let format = if bold {
            strong.clone()
        } else if emphasis {
            italic.clone()
        } else {
            base.clone()
        };
        job.append(&segment, 0.0, format);
    }

    ui.label(job);
}

/// Whether a closing marker exists after `from` (used to avoid treating globs as emphasis).
fn has_closing(chars: &[char], from: usize, marker: &str) -> bool {
    let marker: Vec<char> = marker.chars().collect();
    if marker.is_empty() || from >= chars.len() {
        return false;
    }
    chars[from..]
        .windows(marker.len())
        .any(|window| window == marker)
}

/// `[label](url)` -> (label, url, characters consumed).
fn parse_link(chars: &[char]) -> Option<(String, String, usize)> {
    let close = chars.iter().position(|character| *character == ']')?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = close
        + 2
        + chars[close + 2..]
            .iter()
            .position(|character| *character == ')')?;
    let label: String = chars[1..close].iter().collect();
    let url: String = chars[close + 2..end].iter().collect();
    Some((label, url, end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_and_lists_are_recognised() {
        assert_eq!(heading("## Title"), Some((2, "Title")));
        assert!(heading("not a heading").is_none());
        let item = list_item("- one").unwrap();
        assert_eq!(item.1, "•");
        assert_eq!(item.2, "one");
        assert!(list_item("1. first").is_some());
    }

    #[test]
    fn links_keep_label_and_url() {
        let chars: Vec<char> = "[docs](https://example.com)".chars().collect();
        assert_eq!(
            parse_link(&chars),
            Some((
                "docs".to_string(),
                "https://example.com".to_string(),
                chars.len()
            ))
        );
    }

    #[test]
    fn table_separators_are_detected() {
        assert!(is_table_separator("|---|:---:|"));
        assert!(!is_table_separator("| a | b |"));
    }
}
