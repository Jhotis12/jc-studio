//! Code workspace: file tree, editor, new files and the task's diff.
//!
//! The app reads and writes the repository directly because it runs as the same user; this
//! is editing, not orchestration, so it does not belong in the daemon. Agent work still
//! happens in isolated worktrees, and the editor can open either the project or a task's
//! worktree.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

use egui::{Color32, RichText};

use crate::app::Studio;
use crate::daemon::short;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".next",
    ".cache",
    "graphify-out",
    ".venv",
    "venv",
    "__pycache__",
];
const MAX_FILES: usize = 4_000;
const MAX_DEPTH: usize = 12;
const MAX_EDIT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DIFF_LINES: usize = 4_000;

pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

pub struct OpenFile {
    pub path: PathBuf,
    pub text: String,
    original: String,
}

impl OpenFile {
    pub fn dirty(&self) -> bool {
        self.text != self.original
    }
}

pub struct CodeState {
    pub root: Option<PathBuf>,
    pub root_label: String,
    pub prefer_worktree: bool,
    pub tree: Vec<FileNode>,
    pub open: Option<OpenFile>,
    pub pending_open: Option<PathBuf>,
    pub search: String,
    pub new_path: String,
    pub show_diff: bool,
    /// Set when another view asks for the diff; applied after the root changes.
    pub pending_diff: bool,
    pub diff: Option<String>,
    pub diff_error: Option<String>,
    pub status: Option<(String, bool)>,
    pub commit_message: String,
    pub git_output: Option<(String, bool)>,
}

impl Default for CodeState {
    fn default() -> Self {
        Self {
            root: None,
            root_label: String::new(),
            prefer_worktree: true,
            tree: Vec::new(),
            open: None,
            pending_open: None,
            search: String::new(),
            new_path: String::new(),
            show_diff: false,
            pending_diff: false,
            diff: None,
            diff_error: None,
            status: None,
            commit_message: String::new(),
            git_output: None,
        }
    }
}

impl CodeState {
    pub fn set_root(&mut self, root: PathBuf, label: String) {
        self.root = Some(root);
        self.root_label = label;
        self.open = None;
        self.pending_open = None;
        self.diff = None;
        self.diff_error = None;
        self.status = None;
        self.show_diff = std::mem::take(&mut self.pending_diff);
        self.refresh();
    }

    pub fn refresh(&mut self) {
        let Some(root) = self.root.clone() else {
            self.tree.clear();
            return;
        };
        let mut count = 0;
        self.tree = build_tree(&root, 0, &mut count);
    }

    pub fn open_file(&mut self, path: PathBuf) {
        match read_text(&path) {
            Ok(text) => {
                self.status = None;
                self.show_diff = false;
                self.open = Some(OpenFile {
                    path,
                    original: text.clone(),
                    text,
                });
            }
            Err(error) => self.status = Some((error, true)),
        }
    }

    pub fn save(&mut self) -> bool {
        let Some(file) = self.open.as_mut() else {
            return false;
        };
        match std::fs::write(&file.path, &file.text) {
            Ok(()) => {
                file.original = file.text.clone();
                self.status = Some(("guardado".to_string(), false));
                true
            }
            Err(error) => {
                self.status = Some((format!("no se pudo guardar: {error}"), true));
                false
            }
        }
    }

    pub fn reload(&mut self) {
        let Some(path) = self.open.as_ref().map(|file| file.path.clone()) else {
            return;
        };
        match read_text(&path) {
            Ok(text) => {
                if let Some(file) = self.open.as_mut() {
                    file.original = text.clone();
                    file.text = text;
                }
                self.status = Some(("recargado desde disco".to_string(), false));
            }
            Err(error) => self.status = Some((error, true)),
        }
    }

    pub fn create_file(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let relative = Path::new(self.new_path.trim());
        if relative.as_os_str().is_empty() {
            return;
        }
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
        {
            self.status = Some((
                "la ruta debe ser relativa y no salir del proyecto".into(),
                true,
            ));
            return;
        }
        let path = root.join(relative);
        if path.exists() {
            self.status = Some(("el archivo ya existe".into(), true));
            return;
        }
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                self.status = Some((format!("no se pudo crear la carpeta: {error}"), true));
                return;
            }
        }
        match std::fs::write(&path, "") {
            Ok(()) => {
                self.new_path.clear();
                self.refresh();
                self.open_file(path);
                self.status = Some(("archivo creado".into(), false));
            }
            Err(error) => self.status = Some((format!("no se pudo crear: {error}"), true)),
        }
    }

    /// Stages everything and commits in the given root. Only used for the project checkout:
    /// task worktrees are committed by the orchestrator after verification.
    pub fn commit(&mut self, root: &Path) -> Result<(), String> {
        let message = self.commit_message.trim();
        if message.is_empty() {
            return Err("escribe un mensaje de commit".to_string());
        }
        let add = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["add", "-A"])
            .output()
            .map_err(|error| format!("no se pudo ejecutar git add: {error}"))?;
        if !add.status.success() {
            return Err(String::from_utf8_lossy(&add.stderr).trim().to_string());
        }
        let commit = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["commit", "-m", message])
            .output()
            .map_err(|error| format!("no se pudo ejecutar git commit: {error}"))?;
        let stdout = String::from_utf8_lossy(&commit.stdout).to_string();
        let stderr = String::from_utf8_lossy(&commit.stderr).to_string();
        if commit.status.success() {
            self.commit_message.clear();
            self.git_output = Some((stdout, false));
            self.refresh();
            self.diff = None;
            self.show_diff = false;
            Ok(())
        } else {
            let detail = if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            };
            self.git_output = Some((detail.clone(), true));
            Err(detail.trim().to_string())
        }
    }

    pub fn load_diff(&mut self, base: &str) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let output = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["diff", "--no-color", base])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                self.diff_error = None;
                self.diff = Some(String::from_utf8_lossy(&output.stdout).to_string());
            }
            Ok(output) => {
                self.diff = None;
                self.diff_error = Some(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }
            Err(error) => {
                self.diff = None;
                self.diff_error = Some(format!("no se pudo ejecutar git: {error}"));
            }
        }
    }
}

fn read_text(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    if metadata.len() > MAX_EDIT_BYTES {
        return Err(format!(
            "{} supera el límite de edición de {} MB",
            path.display(),
            MAX_EDIT_BYTES / (1024 * 1024)
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    String::from_utf8(bytes)
        .map_err(|_| format!("{} no es texto UTF-8; usa el terminal", path.display()))
}

fn build_tree(directory: &Path, depth: usize, count: &mut usize) -> Vec<FileNode> {
    if depth > MAX_DEPTH || *count > MAX_FILES {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut nodes = Vec::new();
    for entry in entries.flatten() {
        if *count > MAX_FILES {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            *count += 1;
            nodes.push(FileNode {
                path: entry.path(),
                name,
                is_dir: true,
                children: build_tree(&entry.path(), depth + 1, count),
            });
        } else if file_type.is_file() {
            *count += 1;
            nodes.push(FileNode {
                path: entry.path(),
                name,
                is_dir: false,
                children: Vec::new(),
            });
        }
    }

    nodes.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    nodes
}

fn flatten<'a>(nodes: &'a [FileNode], out: &mut Vec<&'a FileNode>) {
    for node in nodes {
        if !node.is_dir {
            out.push(node);
        }
        flatten(&node.children, out);
    }
}

pub fn panel(ui: &mut egui::Ui, app: &mut Studio) {
    let desired = desired_root(app);
    if let Some((root, label)) = desired {
        if app.code.root.as_ref() != Some(&root) {
            let dirty = app.code.open.as_ref().is_some_and(OpenFile::dirty);
            if dirty {
                app.code.status = Some((
                    "guarda o descarta los cambios antes de cambiar de raíz".to_string(),
                    true,
                ));
            } else {
                app.code.set_root(root, label);
            }
        }
    }

    if app.code.root.is_none() {
        ui.label("Selecciona un proyecto para editar su código.");
        return;
    }

    if app.code.pending_diff {
        app.code.pending_diff = false;
        app.code.show_diff = true;
    }
    if app.code.show_diff && app.code.diff.is_none() && app.code.diff_error.is_none() {
        let base = app
            .state
            .project()
            .map(|project| project.base_branch.clone())
            .unwrap_or_else(|| "main".to_string());
        app.code.load_diff(&base);
    }

    let height = ui.available_height();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(230.0, height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| explorer(ui, app),
        );
        ui.separator();
        ui.vertical(|ui| editor(ui, app));
    });
}

fn desired_root(app: &Studio) -> Option<(PathBuf, String)> {
    let project = app.state.project()?;
    if app.code.prefer_worktree {
        if let Some(task) = app.state.selected_task() {
            if let Some(worktree) = &task.worktree {
                // A merged task's worktree is deleted on purpose. Falling back to the
                // project root beats showing an empty tree with no explanation.
                if Path::new(worktree).exists() {
                    return Some((
                        PathBuf::from(worktree),
                        format!("worktree · {} · {}", short(&task.id), task.status),
                    ));
                }
            }
        }
    }
    Some((PathBuf::from(&project.root_path), project.name.clone()))
}

fn explorer(ui: &mut egui::Ui, app: &mut Studio) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Archivos").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("↻")
                .on_hover_text("volver a leer el árbol")
                .clicked()
            {
                app.code.refresh();
            }
        });
    });
    ui.label(RichText::new(&app.code.root_label).weak().small());

    let worktrees: Vec<(String, String)> = app
        .state
        .tasks
        .iter()
        .filter(|task| {
            task.worktree
                .as_ref()
                .is_some_and(|path| Path::new(path).exists())
        })
        .map(|task| {
            (
                task.id.clone(),
                crate::ui::truncate(&task.title, 34).to_string(),
            )
        })
        .collect();
    let selected_label = app
        .state
        .selected_task()
        .map(|task| crate::ui::truncate(&task.title, 30).to_string())
        .unwrap_or_else(|| "raíz del proyecto".to_string());
    ui.horizontal(|ui| {
        ui.label(RichText::new("Ver").small().weak());
        egui::ComboBox::from_id_salt("code-task")
            .selected_text(selected_label)
            .width(180.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(app.state.selected_task.is_none(), "raíz del proyecto")
                    .clicked()
                {
                    app.state.selected_task = None;
                }
                for (id, title) in &worktrees {
                    if ui
                        .selectable_label(
                            app.state.selected_task.as_deref() == Some(id.as_str()),
                            title,
                        )
                        .clicked()
                    {
                        app.select_task(id.clone());
                    }
                }
            });
    });

    let selected = app.state.selected_task().cloned();
    let missing_worktree = selected.as_ref().is_some_and(|task| {
        task.worktree
            .as_ref()
            .is_some_and(|path| !Path::new(path).exists())
    });
    if missing_worktree {
        ui.label(
            RichText::new(
                "El worktree de esa tarea ya se integró y se eliminó; se muestra la raíz \
                 del proyecto.",
            )
            .small()
            .color(Color32::from_rgb(230, 170, 90)),
        );
    }
    ui.add_enabled_ui(!worktrees.is_empty(), |ui| {
        ui.checkbox(&mut app.code.prefer_worktree, "ver worktree de la tarea");
    });
    if selected.is_some()
        && selected
            .as_ref()
            .is_some_and(|task| task.worktree.is_none())
    {
        ui.label(
            RichText::new("Esta tarea aún no tiene worktree.")
                .small()
                .weak(),
        );
    }

    ui.add_space(4.0);
    ui.add(
        egui::TextEdit::singleline(&mut app.code.search)
            .hint_text("buscar archivo")
            .desired_width(f32::INFINITY),
    );

    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.code.new_path)
                .hint_text("ruta/nuevo.rs")
                .desired_width(140.0),
        );
        if ui.small_button("+ Archivo").clicked() {
            app.code.create_file();
        }
    });

    if let Some((message, error)) = &app.code.status {
        ui.label(RichText::new(message).small().color(if *error {
            Color32::from_rgb(225, 110, 110)
        } else {
            Color32::from_rgb(120, 200, 130)
        }));
    }
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if app.code.search.trim().is_empty() {
                let tree = std::mem::take(&mut app.code.tree);
                for node in &tree {
                    node_ui(ui, app, node, 0);
                }
                app.code.tree = tree;
            } else {
                let query = app.code.search.to_lowercase();
                let mut files = Vec::new();
                flatten(&app.code.tree, &mut files);
                let matches: Vec<(String, PathBuf)> = files
                    .into_iter()
                    .filter_map(|file| {
                        let relative = app
                            .code
                            .root
                            .as_ref()
                            .and_then(|root| file.path.strip_prefix(root).ok())
                            .map(|path| path.to_string_lossy().to_string())
                            .unwrap_or_else(|| file.name.clone());
                        relative
                            .to_lowercase()
                            .contains(&query)
                            .then(|| (relative, file.path.clone()))
                    })
                    .collect();
                for (relative, path) in matches {
                    if ui.selectable_label(false, &relative).clicked() {
                        request_open(app, path);
                    }
                }
            }
        });
}

fn node_ui(ui: &mut egui::Ui, app: &mut Studio, node: &FileNode, depth: usize) {
    if node.is_dir {
        egui::CollapsingHeader::new(RichText::new(&node.name).strong().size(12.5))
            .id_salt(&node.path)
            .default_open(depth == 0)
            .show(ui, |ui| {
                for child in &node.children {
                    node_ui(ui, app, child, depth + 1);
                }
            });
    } else {
        let selected = app
            .code
            .open
            .as_ref()
            .is_some_and(|file| file.path == node.path);
        if ui
            .selectable_label(selected, RichText::new(&node.name).monospace().size(12.5))
            .clicked()
        {
            request_open(app, node.path.clone());
        }
    }
}

fn request_open(app: &mut Studio, path: PathBuf) {
    let dirty = app.code.open.as_ref().is_some_and(OpenFile::dirty);
    if dirty {
        app.code.pending_open = Some(path);
    } else {
        app.code.open_file(path);
    }
}

fn editor(ui: &mut egui::Ui, app: &mut Studio) {
    if let Some(pending) = app.code.pending_open.clone() {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(Color32::from_rgb(230, 170, 90), "Hay cambios sin guardar.");
            if ui.button("Guardar y abrir").clicked() && app.code.save() {
                app.code.pending_open = None;
                app.code.open_file(pending.clone());
            }
            if ui.button("Descartar y abrir").clicked() {
                app.code.pending_open = None;
                app.code.open_file(pending.clone());
            }
            if ui.button("Cancelar").clicked() {
                app.code.pending_open = None;
            }
        });
        ui.separator();
    }

    let Some(path) = app.code.open.as_ref().map(|file| file.path.clone()) else {
        ui.add_space(12.0);
        ui.label(RichText::new("Selecciona un archivo del árbol para editarlo.").weak());
        return;
    };
    let dirty = app.code.open.as_ref().is_some_and(OpenFile::dirty);
    let relative = app
        .code
        .root
        .as_ref()
        .and_then(|root| path.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    ui.horizontal(|ui| {
        ui.label(RichText::new(&relative).monospace().strong());
        if dirty {
            ui.colored_label(Color32::from_rgb(230, 170, 90), "sin guardar");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Guardar").clicked() {
                app.code.save();
            }
            if ui.button("Recargar").clicked() {
                app.code.reload();
            }
            let base = app
                .state
                .project()
                .map(|project| project.base_branch.clone())
                .unwrap_or_else(|| "main".to_string());
            if ui
                .selectable_label(app.code.show_diff, "Cambios")
                .on_hover_text("diff del worktree contra la rama base")
                .clicked()
            {
                app.code.show_diff = !app.code.show_diff;
                if app.code.show_diff {
                    app.code.load_diff(&base);
                }
            }
        });
    });

    if ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S)) {
        app.code.save();
    }

    ui.separator();
    if app.code.show_diff {
        diff_panel(ui, app);
        return;
    }

    if let Some(file) = app.code.open.as_mut() {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut file.text)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(30),
                );
            });
    }

    commit_panel(ui, app, &path);
}

fn commit_panel(ui: &mut egui::Ui, app: &mut Studio, open_file: &Path) {
    let project_root = app
        .state
        .project()
        .map(|project| PathBuf::from(&project.root_path));
    let at_project_root = project_root
        .as_ref()
        .is_some_and(|root| app.code.root.as_ref() == Some(root));

    ui.add_space(8.0);
    egui::CollapsingHeader::new("Commit manual")
        .default_open(false)
        .show(ui, |ui| {
            if !at_project_root {
                ui.label(
                    RichText::new(
                        "Estás en el worktree de una tarea. El orquestador hace el commit                          después de los gates; para commitear a mano, desactiva «usar worktree                          de la tarea» y vuelve a la raíz del proyecto.",
                    )
                    .weak()
                    .small(),
                );
                return;
            }
            ui.label(
                RichText::new(format!(
                    "Se incluirán todos los cambios del repositorio, no solo {}.",
                    open_file
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_default()
                ))
                .weak()
                .small(),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.code.commit_message)
                        .hint_text("mensaje de commit")
                        .desired_width(f32::INFINITY),
                );
                if ui
                    .add_enabled(
                        !app.code.commit_message.trim().is_empty(),
                        egui::Button::new("Commit"),
                    )
                    .clicked()
                {
                    if let Some(root) = &project_root {
                        if let Err(error) = app.code.commit(root) {
                            app.state.notice = Some(crate::state::Notice {
                                message: error,
                                error: true,
                            });
                        }
                    }
                }
            });
            if let Some((output, error)) = &app.code.git_output {
                ui.label(
                    RichText::new(output.trim())
                        .monospace()
                        .small()
                        .color(if *error {
                            Color32::from_rgb(225, 110, 110)
                        } else {
                            Color32::from_rgb(120, 200, 130)
                        }),
                );
            }
        });
}

fn diff_panel(ui: &mut egui::Ui, app: &mut Studio) {
    if let Some(error) = &app.code.diff_error {
        ui.colored_label(Color32::from_rgb(225, 110, 110), error);
        return;
    }
    let Some(diff) = &app.code.diff else {
        ui.label("Sin cambios.");
        return;
    };
    if diff.trim().is_empty() {
        ui.label("Sin cambios respecto a la rama base.");
        return;
    }

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in diff.lines().take(MAX_DIFF_LINES) {
                let color = if line.starts_with("+++") || line.starts_with("---") {
                    Color32::from_rgb(160, 160, 170)
                } else if line.starts_with('+') {
                    Color32::from_rgb(120, 200, 130)
                } else if line.starts_with('-') {
                    Color32::from_rgb(225, 110, 110)
                } else if line.starts_with("@@") {
                    Color32::from_rgb(105, 165, 235)
                } else {
                    Color32::from_rgb(200, 200, 210)
                };
                ui.label(RichText::new(line).monospace().size(12.0).color(color));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_skips_heavy_directories_and_sorts_files_first() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("src")).unwrap();
        std::fs::create_dir(directory.path().join("target")).unwrap();
        std::fs::write(directory.path().join("README.md"), "# demo").unwrap();
        std::fs::write(directory.path().join("src/main.rs"), "fn main() {}").unwrap();

        let mut count = 0;
        let tree = build_tree(directory.path(), 0, &mut count);
        assert!(tree.iter().any(|node| node.name == "src"));
        assert!(!tree.iter().any(|node| node.name == "target"));
        assert_eq!(tree[0].name, "src");
    }

    #[test]
    fn a_file_is_read_and_saved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("a.txt");
        std::fs::write(&path, "hola").unwrap();
        let mut state = CodeState::default();
        state.open_file(path.clone());
        assert_eq!(state.open.as_ref().unwrap().text, "hola");
        state.open.as_mut().unwrap().text = "adios".to_string();
        assert!(state.open.as_ref().unwrap().dirty());
        assert!(state.save());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "adios");
        assert!(!state.open.as_ref().unwrap().dirty());
    }

    #[test]
    fn a_manual_commit_from_the_project_root_works() {
        let directory = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(directory.path())
                .args(args)
                .status()
                .unwrap();
            assert!(status.success());
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "test@jcs.local"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(directory.path().join("a.txt"), "hola").unwrap();

        let mut state = CodeState {
            root: Some(directory.path().to_path_buf()),
            commit_message: "prueba de commit".to_string(),
            ..CodeState::default()
        };
        state.commit(directory.path()).unwrap();

        let log = Command::new("git")
            .arg("-C")
            .arg(directory.path())
            .args(["log", "-1", "--format=%s"])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&log.stdout).trim(),
            "prueba de commit"
        );
        assert!(state.commit_message.is_empty());
    }

    #[test]
    fn paths_cannot_escape_the_project() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = CodeState {
            root: Some(directory.path().to_path_buf()),
            new_path: "../escape.txt".to_string(),
            ..CodeState::default()
        };
        state.create_file();
        assert!(state.status.as_ref().unwrap().1);
        assert!(!directory
            .path()
            .parent()
            .unwrap()
            .join("escape.txt")
            .exists());
    }
}
