//! Plan graph: tasks as nodes, dependencies as arrows, stage progress inside each node.
//!
//! The layout is a column per dependency level. Plans are small (8 tasks maximum), so a
//! deterministic layered layout is enough and avoids a graph-layout dependency.

use std::collections::HashMap;

use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use jcs_proto::TaskView;

use crate::app::Studio;
use crate::state::task_progress;
use crate::ui::{status_color, truncate};

const NODE_WIDTH: f32 = 168.0;
const NODE_HEIGHT: f32 = 58.0;
const GAP_X: f32 = 52.0;
const GAP_Y: f32 = 12.0;
const PAD: f32 = 8.0;

pub fn graph(ui: &mut egui::Ui, app: &mut Studio, tasks: &[TaskView]) {
    if tasks.is_empty() {
        ui.label("Sin tareas todavía.");
        return;
    }

    let by_id: HashMap<&str, &TaskView> =
        tasks.iter().map(|task| (task.id.as_str(), task)).collect();
    let mut memo: HashMap<&str, usize> = HashMap::new();
    let mut visiting: Vec<&str> = Vec::new();
    for task in tasks {
        level(task.id.as_str(), &by_id, &mut memo, &mut visiting);
    }

    let max_level = memo.values().copied().max().unwrap_or(0);
    let mut columns: Vec<Vec<&TaskView>> = vec![Vec::new(); max_level + 1];
    for task in tasks {
        let level = memo.get(task.id.as_str()).copied().unwrap_or(0);
        columns[level].push(task);
    }
    let rows = columns.iter().map(Vec::len).max().unwrap_or(1);

    let width = PAD * 2.0 + (max_level + 1) as f32 * NODE_WIDTH + max_level as f32 * GAP_X;
    let height = PAD * 2.0 + rows as f32 * NODE_HEIGHT + rows.saturating_sub(1) as f32 * GAP_Y;

    egui::ScrollArea::horizontal()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let (response, painter) = ui.allocate_painter(Vec2::new(width, height), Sense::click());
            let origin = response.rect.min + Vec2::splat(PAD);

            let mut rects: HashMap<&str, Rect> = HashMap::new();
            for (column_index, column) in columns.iter().enumerate() {
                let offset_y =
                    (rows.saturating_sub(column.len())) as f32 * (NODE_HEIGHT + GAP_Y) / 2.0;
                for (row, task) in column.iter().enumerate() {
                    let min = origin
                        + Vec2::new(
                            column_index as f32 * (NODE_WIDTH + GAP_X),
                            offset_y + row as f32 * (NODE_HEIGHT + GAP_Y),
                        );
                    rects.insert(
                        task.id.as_str(),
                        Rect::from_min_size(min, Vec2::new(NODE_WIDTH, NODE_HEIGHT)),
                    );
                }
            }

            // Edges first so nodes cover their endpoints.
            for task in tasks {
                let Some(target) = rects.get(task.id.as_str()).copied() else {
                    continue;
                };
                for dependency in &task.depends_on {
                    let Some(source) = rects.get(dependency.as_str()).copied() else {
                        continue;
                    };
                    let start = source.right_center();
                    let end = target.left_center();
                    let control = Vec2::new(24.0, 0.0);
                    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                        [start, start + control, end - control, end],
                        false,
                        Color32::TRANSPARENT,
                        Stroke::new(1.2_f32, Color32::from_gray(95)),
                    ));
                    let arrow = vec![
                        egui::pos2(end.x - 6.0, end.y - 4.0),
                        end,
                        egui::pos2(end.x - 6.0, end.y + 4.0),
                    ];
                    painter.add(egui::Shape::line(
                        arrow,
                        Stroke::new(1.2_f32, Color32::from_gray(95)),
                    ));
                }
            }

            let pointer = ui.ctx().pointer_hover_pos();
            for task in tasks {
                let Some(rect) = rects.get(task.id.as_str()).copied() else {
                    continue;
                };
                let progress = task_progress(task);
                let color = status_color(&task.status);
                let selected = app.state.selected_task.as_deref() == Some(task.id.as_str());
                let is_hovered = pointer.is_some_and(|position| rect.contains(position));

                painter.rect_filled(rect, CornerRadius::same(6), color.gamma_multiply(0.12));
                painter.rect_stroke(
                    rect,
                    CornerRadius::same(6),
                    Stroke::new(
                        if selected || is_hovered {
                            2.0_f32
                        } else {
                            1.0_f32
                        },
                        if is_hovered {
                            color.gamma_multiply(1.3)
                        } else {
                            color
                        },
                    ),
                    StrokeKind::Inside,
                );

                painter.text(
                    rect.min + Vec2::new(9.0, 7.0),
                    Align2::LEFT_TOP,
                    truncate(&task.title, 22),
                    FontId::proportional(12.0),
                    ui.visuals().strong_text_color(),
                );
                painter.text(
                    rect.min + Vec2::new(9.0, 25.0),
                    Align2::LEFT_TOP,
                    format!(
                        "{}% · {}",
                        (progress.fraction * 100.0).round() as i32,
                        progress.label
                    ),
                    FontId::proportional(10.0),
                    color,
                );

                let bar = Rect::from_min_size(
                    rect.left_bottom() + Vec2::new(9.0, -12.0),
                    Vec2::new(rect.width() - 18.0, 3.0),
                );
                painter.rect_filled(bar, CornerRadius::same(2), Color32::from_gray(48));
                painter.rect_filled(
                    Rect::from_min_size(bar.min, Vec2::new(bar.width() * progress.fraction, 3.0)),
                    CornerRadius::same(2),
                    color,
                );

                if is_hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }

            if response.clicked() {
                if let Some(position) = response.interact_pointer_pos() {
                    for task in tasks {
                        if rects
                            .get(task.id.as_str())
                            .is_some_and(|rect| rect.contains(position))
                        {
                            app.focus_task(task.goal_id.clone(), task.id.clone());
                            break;
                        }
                    }
                }
            }
        });
}

/// Longest path from a task with no dependencies. The plan is validated acyclic, but the
/// `visiting` guard keeps a corrupt plan from recursing forever.
fn level<'a>(
    id: &'a str,
    by_id: &HashMap<&'a str, &'a TaskView>,
    memo: &mut HashMap<&'a str, usize>,
    visiting: &mut Vec<&'a str>,
) -> usize {
    if let Some(level) = memo.get(id) {
        return *level;
    }
    if visiting.contains(&id) {
        return 0;
    }
    let Some(task) = by_id.get(id) else {
        return 0;
    };
    visiting.push(id);
    let level = task
        .depends_on
        .iter()
        .map(|dependency| level(dependency.as_str(), by_id, memo, visiting) + 1)
        .max()
        .unwrap_or(0);
    visiting.pop();
    memo.insert(id, level);
    level
}
