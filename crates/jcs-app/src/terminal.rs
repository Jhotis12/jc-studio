//! Embedded terminals.
//!
//! A real PTY plus a VT100 parser: enough to run `opencode`, `git`, or a test suite inside
//! the application, in the directory of a project or a task worktree. Sessions live in this
//! process; the daemon-side PTY (so a session survives closing the window) is the next step.

use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use egui::{Align2, Color32, FontId, Sense, Stroke};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

/// A terminal session: the PTY, its parser and the reader channel.
struct Session {
    title: String,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    parser: vt100::Parser,
    output: Receiver<Vec<u8>>,
    rows: u16,
    cols: u16,
    exited: bool,
    /// Rows scrolled up from the live screen.
    scrollback: usize,
}

impl Session {
    fn resize(&mut self, rows: u16, cols: u16) {
        if rows == self.rows && cols == self.cols {
            return;
        }
        self.rows = rows.max(1);
        self.cols = cols.max(1);
        self.parser.set_size(nonzero(self.rows), nonzero(self.cols));
        let _ = self.master.resize(PtySize {
            rows: self.rows,
            cols: self.cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.scrollback = 0;
        self.parser.screen_mut().set_scrollback(0);
    }

    fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }
}

#[derive(Default)]
pub struct Terminals {
    sessions: Vec<Session>,
    active: usize,
}

impl Terminals {
    /// Drains PTY output into the parsers and notices dead children.
    pub fn poll(&mut self) {
        for session in &mut self.sessions {
            while let Ok(chunk) = session.output.try_recv() {
                session.parser.process(&chunk);
            }
            if !session.exited && matches!(session.child.try_wait(), Ok(Some(_))) {
                session.exited = true;
            }
        }
    }

    pub fn open(&mut self, cwd: PathBuf, title: String) -> anyhow::Result<()> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut command = CommandBuilder::new(&shell);
        command.cwd(&cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        self.spawn(command, title)
    }

    fn spawn(&mut self, command: CommandBuilder, title: String) -> anyhow::Result<()> {
        let pair = native_pty_system().openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let child = pair.slave.spawn_command(command)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let (sender, output) = channel();
        std::thread::Builder::new()
            .name("jcs-terminal-reader".to_string())
            .spawn(move || {
                let mut buffer = [0u8; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => {
                            if sender.send(buffer[..count].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                }
            })?;

        self.sessions.push(Session {
            title,
            master: pair.master,
            writer,
            child,
            parser: vt100::Parser::new(nonzero(24), nonzero(80), 2000),
            output,
            rows: 24,
            cols: 80,
            exited: false,
            scrollback: 0,
        });
        self.active = self.sessions.len() - 1;
        Ok(())
    }

    pub fn close_active(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let mut session = self.sessions.remove(self.active);
        let _ = session.child.kill();
        self.active = self.active.min(self.sessions.len().saturating_sub(1));
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Renders the terminal dock. `cwd` is where a new session starts (project or worktree).
    pub fn ui(&mut self, ui: &mut egui::Ui, cwd: Option<PathBuf>) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Terminales").strong());
            ui.separator();
            for index in 0..self.sessions.len() {
                let selected = index == self.active;
                let title = self.sessions[index].title.clone();
                if ui
                    .selectable_label(selected, format!("{title} · #{}", index + 1))
                    .clicked()
                {
                    self.active = index;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.sessions.is_empty() && ui.small_button("Cerrar").clicked() {
                    self.close_active();
                }
                if ui.small_button("+ Nueva").clicked() {
                    let directory = cwd.clone().unwrap_or_else(|| PathBuf::from("."));
                    let title = directory
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| "shell".to_string());
                    let _ = self.open(directory, title);
                }
            });
        });

        if self.sessions.is_empty() {
            ui.add_space(12.0);
            ui.label("No hay ninguna terminal abierta. Se abrirá en el proyecto seleccionado.");
            return;
        }

        let session = &mut self.sessions[self.active];
        render(ui, session);
    }
}

/// Draws the screen grid, handles keyboard input and resizes the PTY to the visible area.
fn render(ui: &mut egui::Ui, session: &mut Session) {
    let font = FontId::monospace(13.0);
    let (cell_width, cell_height) = ui.fonts(|fonts| {
        (
            fonts.glyph_width(&font, 'M').max(6.0),
            fonts.row_height(&font).max(12.0),
        )
    });

    let available = ui.available_size();
    let cols = ((available.x - 8.0) / cell_width).floor().max(20.0) as u16;
    let rows = ((available.y - 8.0) / cell_height).floor().max(5.0) as u16;
    session.resize(rows, cols);

    let size = egui::vec2(
        cols as f32 * cell_width + 2.0,
        rows as f32 * cell_height + 2.0,
    );
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let origin = response.rect.min + egui::vec2(1.0, 1.0);

    // Scrolling walks the scrollback. The live screen is offset zero.
    if response.hovered() {
        let delta = ui.input(|input| input.smooth_scroll_delta.y);
        if delta.abs() > 0.1 {
            let capacity = session.parser.screen().scrollback_len();
            let next = if delta > 0.0 {
                (session.scrollback + (delta / cell_height).ceil() as usize).min(capacity)
            } else {
                session
                    .scrollback
                    .saturating_sub((-delta / cell_height).ceil() as usize)
            };
            if next != session.scrollback {
                session.scrollback = next;
                session.parser.screen_mut().set_scrollback(next);
            }
        }
    }

    if response.clicked() {
        response.request_focus();
    }

    let default_fg = Color32::from_rgb(220, 224, 232);
    let default_bg = Color32::from_rgb(14, 16, 20);
    painter.rect_filled(response.rect, 0.0, default_bg);

    let screen = session.parser.screen();
    for row in 0..rows {
        for col in 0..cols {
            let Some(cell) = screen.cell(row, col) else {
                continue;
            };
            if cell.is_wide_continuation() {
                continue;
            }
            let width = if cell.is_wide() { 2.0 } else { 1.0 };
            let rect = egui::Rect::from_min_size(
                origin + egui::vec2(col as f32 * cell_width, row as f32 * cell_height),
                egui::vec2(width * cell_width, cell_height),
            );

            let mut fg = color(cell.fgcolor(), default_fg);
            let mut bg = color(cell.bgcolor(), default_bg);
            if cell.inverse() {
                std::mem::swap(&mut fg, &mut bg);
                if bg == default_bg {
                    bg = default_fg;
                }
                if fg == default_fg {
                    fg = default_bg;
                }
            }
            if bg != default_bg {
                painter.rect_filled(rect, 0.0, bg);
            }

            let contents = cell.contents();
            if !contents.is_empty() && contents != " " {
                painter.text(rect.min, Align2::LEFT_TOP, contents, font.clone(), fg);
            }
            if cell.underline() {
                painter.line_segment(
                    [
                        egui::pos2(rect.min.x, rect.max.y - 1.0),
                        egui::pos2(rect.max.x, rect.max.y - 1.0),
                    ],
                    Stroke::new(1.0_f32, fg),
                );
            }
        }
    }

    if !screen.hide_cursor() && session.scrollback == 0 && response.has_focus() {
        let (row, col) = screen.cursor_position();
        if row < rows && col < cols {
            let rect = egui::Rect::from_min_size(
                origin + egui::vec2(col as f32 * cell_width, row as f32 * cell_height),
                egui::vec2(cell_width, cell_height),
            );
            painter.rect_filled(rect, 0.0, default_fg.gamma_multiply(0.7));
        }
    }

    // Keyboard. `Event::Text` already excludes control characters.
    if response.has_focus() {
        let events = ui.input(|input| input.events.clone());
        let modifiers = ui.input(|input| input.modifiers);
        let mut bytes = Vec::new();
        for event in events {
            match event {
                egui::Event::Text(text) => bytes.extend_from_slice(text.as_bytes()),
                egui::Event::Paste(text) => bytes.extend_from_slice(text.as_bytes()),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    // These two have dedicated meanings: paste arrives as its own event and
                    // copy is Ctrl+Shift+C. Sending them as control bytes would double them.
                    if modifiers.command && matches!(key, egui::Key::V)
                        || modifiers.command && modifiers.shift && matches!(key, egui::Key::C)
                    {
                        continue;
                    }
                    match key {
                        egui::Key::Enter => bytes.extend_from_slice(b"\r"),
                        egui::Key::Backspace => bytes.extend_from_slice(b"\x7f"),
                        egui::Key::Tab => bytes.extend_from_slice(b"\t"),
                        egui::Key::Escape => bytes.extend_from_slice(b"\x1b"),
                        egui::Key::ArrowUp => bytes.extend_from_slice(b"\x1b[A"),
                        egui::Key::ArrowDown => bytes.extend_from_slice(b"\x1b[B"),
                        egui::Key::ArrowRight => bytes.extend_from_slice(b"\x1b[C"),
                        egui::Key::ArrowLeft => bytes.extend_from_slice(b"\x1b[D"),
                        egui::Key::Home => bytes.extend_from_slice(b"\x1b[H"),
                        egui::Key::End => bytes.extend_from_slice(b"\x1b[F"),
                        egui::Key::Delete => bytes.extend_from_slice(b"\x1b[3~"),
                        egui::Key::PageUp => bytes.extend_from_slice(b"\x1b[5~"),
                        egui::Key::PageDown => bytes.extend_from_slice(b"\x1b[6~"),
                        _ => {
                            if modifiers.ctrl && !modifiers.shift {
                                if let Some(letter) = key.name().chars().next() {
                                    if letter.is_ascii_alphabetic() {
                                        let control = (letter.to_ascii_lowercase() as u8) & 0x1f;
                                        bytes.push(control);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if !bytes.is_empty() {
            session.write(&bytes);
        }

        if modifiers.command && modifiers.shift && ui.input(|input| input.key_pressed(egui::Key::C))
        {
            ui.ctx().copy_text(session.parser.screen().contents());
        }
    }

    if session.exited {
        painter.text(
            response.rect.right_bottom() - egui::vec2(8.0, 8.0),
            Align2::RIGHT_BOTTOM,
            "proceso terminado",
            FontId::monospace(11.0),
            Color32::from_rgb(200, 120, 120),
        );
    }
}

fn nonzero(value: u16) -> NonZeroU16 {
    NonZeroU16::new(value.max(1)).expect("clamped to one")
}

/// Maps a parsed colour to something readable on the dark terminal background.
fn color(value: vt100::Color, default: Color32) -> Color32 {
    match value {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        vt100::Color::Idx(index) => {
            if index < 16 {
                const PALETTE: [Color32; 16] = [
                    Color32::from_rgb(24, 26, 32),
                    Color32::from_rgb(224, 108, 117),
                    Color32::from_rgb(152, 195, 121),
                    Color32::from_rgb(229, 192, 123),
                    Color32::from_rgb(97, 175, 239),
                    Color32::from_rgb(198, 120, 221),
                    Color32::from_rgb(86, 182, 194),
                    Color32::from_rgb(200, 204, 212),
                    Color32::from_rgb(92, 99, 112),
                    Color32::from_rgb(255, 141, 148),
                    Color32::from_rgb(190, 230, 160),
                    Color32::from_rgb(255, 226, 160),
                    Color32::from_rgb(140, 200, 255),
                    Color32::from_rgb(230, 160, 255),
                    Color32::from_rgb(130, 220, 230),
                    Color32::from_rgb(240, 244, 250),
                ];
                PALETTE[index as usize]
            } else if index < 232 {
                let index = index - 16;
                let steps = [0u8, 95, 135, 175, 215, 255];
                Color32::from_rgb(
                    steps[(index / 36) as usize],
                    steps[((index % 36) / 6) as usize],
                    steps[(index % 6) as usize],
                )
            } else {
                let level = 8 + (index - 232) * 10;
                Color32::from_gray(level)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_command_runs_and_reaches_the_screen() {
        let mut terminals = Terminals::default();
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "printf 'hola desde el pty\\n'"]);
        terminals.spawn(command, "prueba".to_string()).unwrap();

        let mut seen = false;
        for _ in 0..200 {
            terminals.poll();
            if terminals.sessions[0]
                .parser
                .screen()
                .contents()
                .contains("hola desde el pty")
            {
                seen = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            seen,
            "{:?}",
            terminals.sessions[0].parser.screen().contents()
        );
    }

    #[test]
    fn the_256_colour_cube_maps_to_rgb() {
        assert_eq!(
            color(vt100::Color::Idx(196), Color32::BLACK),
            Color32::from_rgb(255, 0, 0)
        );
        assert_eq!(
            color(vt100::Color::Idx(231), Color32::BLACK),
            Color32::from_rgb(255, 255, 255)
        );
    }
}
