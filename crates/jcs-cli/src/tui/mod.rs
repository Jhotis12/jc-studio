//! Terminal user interface.
//!
//! A thin client like the rest of `jcs`: it reads snapshots and an event stream from the
//! daemon and sends commands back. Nothing durable lives here, so killing the TUI cannot
//! lose work.

mod app;
mod ui;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use jcs_proto::{AskRequest, PermissionMode};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use tokio::sync::mpsc;

use crate::client::Client;
use app::{Action, App};

/// Snapshot refresh interval. Events keep the interface live between refreshes.
const REFRESH: Duration = Duration::from_secs(2);
/// Frame pacing; also yields to the runtime.
const FRAME: Duration = Duration::from_millis(50);

pub fn run(client: Client) -> Result<()> {
    // The TUI owns its runtime: the terminal loop polls input, and nothing else must be
    // starved by it.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("cannot start the TUI runtime")?;

    runtime.block_on(async move {
        let mut terminal = ratatui::init();
        let result = event_loop(&mut terminal, client).await;
        ratatui::restore();
        result
    })
}

async fn event_loop(terminal: &mut ratatui::DefaultTerminal, client: Client) -> Result<()> {
    let mut app = App::default();

    if let Err(error) = client.status().await {
        app.last_error = Some(format!("{error:#}"));
    } else if let Ok(status) = client.status().await {
        app.refresh(status);
    }

    // Follow only new events: replaying all history into a 300-line pane is noise.
    let cursor = client.latest_seq().await.unwrap_or(0);
    let mut events = spawn_event_stream(client.clone(), cursor);
    let mut last_refresh = Instant::now();

    loop {
        while event::poll(Duration::ZERO).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => handle_key(&mut app, key),
                Ok(_) => {}
                Err(_) => {}
            }
        }

        while let Ok(event) = events.try_recv() {
            app.apply_event(event);
        }

        if last_refresh.elapsed() >= REFRESH {
            match client.status().await {
                Ok(status) => {
                    app.refresh(status);
                    app.last_error = None;
                }
                Err(error) => app.last_error = Some(format!("{error:#}")),
            }
            last_refresh = Instant::now();
        }

        terminal.draw(|frame| ui::draw(frame, &app))?;

        if app.quit {
            return Ok(());
        }

        if let Some(action) = app.pending.take() {
            execute(&client, action, &mut app).await;
        }

        tokio::time::sleep(FRAME).await;
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    if app.input.is_some() {
        match key.code {
            KeyCode::Esc => app.input = None,
            KeyCode::Enter => app.submit_input(),
            KeyCode::Backspace => {
                if let Some(buffer) = app.input.as_mut() {
                    buffer.pop();
                }
            }
            KeyCode::Char(character) => {
                if let Some(buffer) = app.input.as_mut() {
                    buffer.push(character);
                }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Tab => app.cycle_focus(),
        KeyCode::Char('j') | KeyCode::Down => app.move_selection(1),
        KeyCode::Char('k') | KeyCode::Up => app.move_selection(-1),
        KeyCode::Char('a') => app.begin_ask(),
        KeyCode::Char('r') => app.request_render(),
        KeyCode::Char('s') => app.request_resume(),
        _ => {}
    }
}

async fn execute(client: &Client, action: Action, app: &mut App) {
    match action {
        Action::Ask { project, prompt } => {
            let request = AskRequest {
                project,
                prompt,
                model: None,
                agent: None,
                permissions: PermissionMode::Auto,
                title: None,
                plan: true,
            };
            match client.ask(&request).await {
                Ok(response) => app.toast(format!("goal {}", app::short(&response.goal_id))),
                Err(error) => {
                    app.toast("ask failed");
                    app.last_error = Some(format!("{error:#}"));
                }
            }
        }
        Action::RenderPolicy { project } => {
            let request = jcs_proto::RenderPolicyRequest {
                project,
                check: false,
                force: false,
            };
            match client.render_policy(&request).await {
                Ok(response) => {
                    let changed = response
                        .files
                        .iter()
                        .filter(|file| file.state != "unchanged")
                        .count();
                    app.toast(format!("policy: {changed} file(s) changed"));
                }
                Err(error) => {
                    app.toast("render failed");
                    app.last_error = Some(format!("{error:#}"));
                }
            }
        }
        Action::ResumeGoal { goal_id } => match client.resume_goal(&goal_id).await {
            Ok(_) => app.toast("goal resumed"),
            Err(error) => {
                app.toast("resume failed");
                app.last_error = Some(format!("{error:#}"));
            }
        },
    }
}

fn spawn_event_stream(client: Client, since: i64) -> mpsc::UnboundedReceiver<jcs_core::Event> {
    let (sender, receiver) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut cursor = since;

        loop {
            if let Ok(mut stream) = client.events(cursor).await {
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(event) => {
                            cursor = event.seq;
                            if sender.send(event).is_err() {
                                return;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }

            // The daemon may have restarted; reconnect from the last event we saw.
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    receiver
}
