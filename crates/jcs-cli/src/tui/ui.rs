//! Rendering. Pure: it reads `App` and writes to a frame, nothing else.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{status_style, App, Focus};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let outer = Layout::vertical([
        Constraint::Min(8),
        Constraint::Length(9),
        Constraint::Length(1),
    ])
    .split(area);

    let columns = Layout::horizontal([
        Constraint::Length(26),
        Constraint::Length(44),
        Constraint::Min(20),
    ])
    .split(outer[0]);

    draw_projects(frame, app, columns[0]);
    draw_goals(frame, app, columns[1]);
    draw_details(frame, app, columns[2]);
    draw_log(frame, app, outer[1]);
    draw_status(frame, app, outer[2]);

    if app.input.is_some() {
        draw_input(frame, app, area);
    }
}

fn focused(app: &App, focus: Focus) -> Style {
    if app.focus == focus {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::Gray)
    }
}

fn draw_projects(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .projects
        .iter()
        .map(|project| {
            let merges = if project.auto_merge { " ⤳" } else { "" };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<18}", truncate(&project.name, 18)),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::styled(merges, Style::new().fg(Color::Green)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title(Span::styled(" Projects ", focused(app, Focus::Projects))))
        .highlight_style(Style::new().reversed())
        .highlight_symbol("› ");

    let mut state = ListState::default().with_selected(Some(app.project_index));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_goals(frame: &mut Frame, app: &App, area: Rect) {
    let halves =
        Layout::vertical([Constraint::Percentage(45), Constraint::Percentage(55)]).split(area);
    let goals = app.goals_for_selected_project();

    let items: Vec<ListItem> = goals
        .iter()
        .map(|goal| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<10}", truncate(&goal.status, 10)),
                    status_style(&goal.status),
                ),
                Span::raw(truncate(&goal.prompt, 30)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title(Span::styled(" Goals ", focused(app, Focus::Goals))))
        .highlight_style(Style::new().reversed())
        .highlight_symbol("› ");

    let mut state = ListState::default().with_selected(Some(app.goal_index));
    frame.render_stateful_widget(list, halves[0], &mut state);

    draw_tasks(frame, app, halves[1]);
}

fn draw_tasks(frame: &mut Frame, app: &App, area: Rect) {
    let tasks = app
        .selected_goal()
        .map(|goal| app.tasks_for_goal(&goal.id))
        .unwrap_or_default();

    let items: Vec<ListItem> = tasks
        .iter()
        .map(|task| {
            let deps = if task.depends_on.is_empty() {
                String::new()
            } else {
                format!(
                    " ⤶{}",
                    task.depends_on
                        .iter()
                        .map(|id| super::app::short(id))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<9}", truncate(&task.status, 9)),
                    status_style(&task.status),
                ),
                Span::raw(truncate(&task.title, 22)),
                Span::styled(deps, Style::new().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title(Span::styled(" Tasks ", focused(app, Focus::Tasks))));

    frame.render_widget(list, area);
}

fn draw_details(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered().title(" Details ");
    let Some(goal) = app.selected_goal() else {
        let empty = Paragraph::new("No goal selected.\n\nPress `a` to send an objective.")
            .block(block)
            .wrap(Wrap { trim: true });
        frame.render_widget(empty, area);
        return;
    };

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("status  ", Style::new().fg(Color::DarkGray)),
            Span::styled(goal.status.clone(), status_style(&goal.status)),
        ]),
        Line::from(vec![
            Span::styled("goal    ", Style::new().fg(Color::DarkGray)),
            Span::raw(super::app::short(&goal.id)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            goal.prompt.clone(),
            Style::new().add_modifier(Modifier::ITALIC),
        )),
    ];

    let tasks = app.tasks_for_goal(&goal.id);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("tasks ({})", tasks.len()),
        Style::new().fg(Color::DarkGray),
    )));
    for task in tasks {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<9}", truncate(&task.status, 9)),
                status_style(&task.status),
            ),
            Span::raw(truncate(&task.title, 34)),
        ]));
        for run in app.runs_for_task(&task.id) {
            lines.push(Line::from(Span::styled(
                format!(
                    "    run {} · ${:.4}{}",
                    run.status,
                    run.cost_usd,
                    run.error
                        .as_deref()
                        .map(|error| format!(" · {}", truncate(error, 40)))
                        .unwrap_or_default()
                ),
                Style::new().fg(Color::DarkGray),
            )));
        }
    }

    if let Some(error) = &app.last_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("error: {}", truncate(error, 200)),
            Style::new().fg(Color::Red),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_log(frame: &mut Frame, app: &App, area: Rect) {
    let height = area.height.saturating_sub(2) as usize;
    let start = app.log.len().saturating_sub(height);
    let lines: Vec<Line> = app
        .log
        .iter()
        .skip(start)
        .map(|line| Line::from(line.as_str()))
        .collect();

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(" Events "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let backend = app
        .health
        .as_ref()
        .map(|health| {
            if health.agent.reachable {
                format!(
                    "{} {}",
                    health.agent.backend,
                    health.agent.version.clone().unwrap_or_default()
                )
            } else {
                format!("{} UNREACHABLE", health.agent.backend)
            }
        })
        .unwrap_or_else(|| "daemon unreachable".to_string());

    let toast = app.live_toast().unwrap_or("");
    let hints = "q quit · tab focus · j/k move · a ask · r render · s resume";

    let line = Line::from(vec![
        Span::styled(format!(" {backend} "), Style::new().fg(Color::Cyan)),
        Span::raw(format!(
            "· ${:.4} · {} ",
            app.totals_cost,
            match app.focus {
                Focus::Projects => "projects",
                Focus::Goals => "goals",
                Focus::Tasks => "tasks",
            }
        )),
        Span::styled(toast, Style::new().fg(Color::Yellow)),
        Span::styled(format!("   {hints}"), Style::new().fg(Color::DarkGray)),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let Some(buffer) = &app.input else {
        return;
    };

    let popup = centered_fixed(area, 70, 7);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Objective for the selected project"),
            Line::from(""),
            Line::from(buffer.as_str()),
            Line::from(""),
            Line::from(Span::styled(
                "enter send · esc cancel",
                Style::new().fg(Color::DarkGray),
            )),
        ])
        .block(Block::bordered().title(" New objective "))
        .wrap(Wrap { trim: false }),
        popup,
    );
}

/// A horizontally proportional, vertically fixed popup.
///
/// Fixed height keeps the modal readable on short terminals, where a percentage would
/// collapse to one line and hide the hint.
fn centered_fixed(area: Rect, percent_x: u16, height: u16) -> Rect {
    let height = height.min(area.height);
    let vertical = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(height),
        Constraint::Min(0),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value
        .chars()
        .take(max.saturating_sub(1))
        .collect::<String>()
        + "…"
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    fn rendered(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();

        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn an_empty_dashboard_renders_every_pane() {
        let app = App::default();
        let text = rendered(&app, 100, 30);

        assert!(text.contains("Projects"), "{text}");
        assert!(text.contains("Goals"), "{text}");
        assert!(text.contains("Tasks"), "{text}");
        assert!(text.contains("Details"), "{text}");
        assert!(text.contains("Events"), "{text}");
        assert!(text.contains("No goal selected"), "{text}");
    }

    #[test]
    fn the_selected_goal_and_its_tasks_are_visible() {
        let mut app = App::default();
        let now: jcs_proto::Timestamp = "2026-01-01T00:00:00Z".parse().unwrap();

        app.projects = vec![jcs_proto::ProjectView {
            id: "prj_demo".into(),
            name: "demo".into(),
            root_path: "/tmp/demo".into(),
            base_branch: "main".into(),
            created_at: now,
            auto_merge: true,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        }];
        app.goals = vec![jcs_proto::GoalView {
            id: "goal_1".into(),
            project_id: "prj_demo".into(),
            prompt: "ship the feature".into(),
            status: "running".into(),
            created_at: now,
            updated_at: now,
        }];
        app.tasks = vec![jcs_proto::TaskView {
            id: "task_1".into(),
            goal_id: "goal_1".into(),
            title: "write the code".into(),
            status: "succeeded".into(),
            worktree: None,
            branch: None,
            session_id: None,
            depends_on: vec!["task_0".into()],
            path_claims: Vec::new(),
            merged_at: None,
            created_at: now,
            updated_at: now,
        }];
        app.runs = vec![jcs_proto::RunView {
            id: "run_1".into(),
            task_id: "task_1".into(),
            attempt: 1,
            status: "succeeded".into(),
            exit_code: Some(0),
            started_at: now,
            ended_at: Some(now),
            cost_usd: 0.0125,
            tokens_in: 100,
            tokens_out: 20,
            error: None,
        }];

        let text = rendered(&app, 120, 40);
        assert!(text.contains("demo"), "{text}");
        assert!(text.contains("ship the feature"), "{text}");
        assert!(text.contains("write the code"), "{text}");
        assert!(text.contains("succeeded"), "{text}");
        // Dependencies are shown on the task line.
        assert!(text.contains("⤶"), "{text}");
    }

    #[test]
    fn the_input_modal_is_rendered_when_open() {
        let app = App {
            input: Some("add validation".into()),
            ..App::default()
        };

        let text = rendered(&app, 100, 30);
        assert!(text.contains("New objective"), "{text}");
        assert!(text.contains("add validation"), "{text}");
        assert!(text.contains("esc cancel"), "{text}");
    }
}
