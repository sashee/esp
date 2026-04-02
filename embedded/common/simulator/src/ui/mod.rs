use std::io;
use std::path::Path;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Cell;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;
use time::format_description::BorrowedFormatItem;
use time::{macros::format_description, OffsetDateTime};

use crate::editor::{
    open_or_create_trace, replay_state_at, save_runtime_replay, save_trace, snapshot_for, update,
    AppState, Command, EditorSession, Effect, ReplayEnvelope, RuntimeTraceItem, TraceRuntime,
    TraceViewDialog, ViewSnapshot,
};

const REPLAY_TIMESTAMP_FORMAT: &[BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day]_[hour]-[minute]-[second]");

struct ReplayRecorder<T> {
    initial_state: AppState<T>,
    commands: Vec<Command>,
}

enum LiveAction {
    Command(Command),
    SaveReplay,
}

enum ReplayAction {
    Prev,
    Next,
    Quit,
}

pub fn run_editor<R: TraceRuntime>(runtime: &R, path: &Path) -> Result<(), String> {
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide).map_err(|err| err.to_string())?;
    terminal::enable_raw_mode().map_err(|err| err.to_string())?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| err.to_string())?;
    let size = terminal.size().map_err(|err| err.to_string())?;
    let mut session = open_or_create_trace(runtime, path, size.width, size.height)?;
    let mut recorder = ReplayRecorder {
        initial_state: session.state.clone(),
        commands: Vec::new(),
    };

    let result = run_live_loop(&mut terminal, &mut session, &mut recorder, runtime);

    let cleanup_result = (|| -> Result<(), String> {
        terminal::disable_raw_mode().map_err(|err| err.to_string())?;
        execute!(terminal.backend_mut(), Show, LeaveAlternateScreen)
            .map_err(|err| err.to_string())?;
        Ok(())
    })();

    result.and(cleanup_result)
}

pub fn run_replay<R: TraceRuntime>(
    runtime: &R,
    replay_path: &Path,
    replay: ReplayEnvelope<RuntimeTraceItem<R>>,
) -> Result<(), String> {
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide).map_err(|err| err.to_string())?;
    terminal::enable_raw_mode().map_err(|err| err.to_string())?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| err.to_string())?;
    let result = run_replay_loop(&mut terminal, replay_path, replay, runtime);

    let cleanup_result = (|| -> Result<(), String> {
        terminal::disable_raw_mode().map_err(|err| err.to_string())?;
        execute!(terminal.backend_mut(), Show, LeaveAlternateScreen)
            .map_err(|err| err.to_string())?;
        Ok(())
    })();

    result.and(cleanup_result)
}

fn run_live_loop<R: TraceRuntime>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    session: &mut EditorSession<
        crate::editor::TraceItem<
            R::SyncOp,
            R::AsyncOp,
            R::SyncResult,
            R::SyncError,
            R::AsyncResult,
        >,
    >,
    recorder: &mut ReplayRecorder<RuntimeTraceItem<R>>,
    runtime: &R,
) -> Result<(), String> {
    loop {
        terminal
            .draw(|frame| render(frame, session, runtime))
            .map_err(|err| err.to_string())?;
        let event = event::read().map_err(|err| err.to_string())?;
        let Some(action) = map_event_to_live_action(&session.state, event) else {
            continue;
        };
        match action {
            LiveAction::SaveReplay => {
                match save_replay_snapshot(&session.path, recorder, runtime) {
                    Ok(path) => {
                        session.state.view.status = Some(format!("saved replay {}", path.display()))
                    }
                    Err(err) => session.state.view.status = Some(err),
                }
            }
            LiveAction::Command(command) => {
                if !matches!(command, Command::Quit) {
                    recorder.commands.push(command.clone());
                }
                let state = std::mem::take(&mut session.state);
                let (next_state, effects) = update(state, command, runtime);
                session.state = next_state;
                for effect in effects {
                    match effect {
                        Effect::SaveTrace { trace } => {
                            match save_trace(runtime, &session.path, &trace) {
                                Ok(()) => {
                                    session.state.view.status =
                                        Some(format!("saved {}", session.path.display()))
                                }
                                Err(err) => session.state.view.status = Some(err),
                            }
                        }
                        Effect::Quit => return Ok(()),
                    }
                }
            }
        }
    }
}

fn run_replay_loop<R: TraceRuntime>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    replay_path: &Path,
    replay: ReplayEnvelope<RuntimeTraceItem<R>>,
    runtime: &R,
) -> Result<(), String> {
    let mut state_index = 0usize;
    loop {
        let state = replay_state_at(
            runtime,
            &replay.initial_state,
            &replay.commands,
            state_index,
        )?;
        terminal
            .draw(|frame| {
                render_replay(
                    frame,
                    replay_path,
                    &state,
                    &replay.commands,
                    state_index,
                    runtime,
                )
            })
            .map_err(|err| err.to_string())?;
        let event = event::read().map_err(|err| err.to_string())?;
        let Some(action) = map_event_to_replay_action(event) else {
            continue;
        };
        match action {
            ReplayAction::Prev => {
                state_index = state_index.saturating_sub(1);
            }
            ReplayAction::Next => {
                if state_index < replay.commands.len() {
                    state_index += 1;
                }
            }
            ReplayAction::Quit => return Ok(()),
        }
    }
}

fn map_event_to_live_action<T>(
    state: &crate::editor::AppState<T>,
    event: Event,
) -> Option<LiveAction> {
    match event {
        Event::Resize(width, height) => {
            Some(LiveAction::Command(Command::Resize { width, height }))
        }
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            let in_choice_dialog = matches!(&state.view.dialog, TraceViewDialog::Choice { .. });
            let in_form_dialog = matches!(&state.view.dialog, TraceViewDialog::Form { .. });
            if in_choice_dialog {
                return match key.code {
                    KeyCode::Esc => Some(LiveAction::Command(Command::DialogCancel)),
                    KeyCode::Enter => Some(LiveAction::Command(Command::DialogConfirm)),
                    KeyCode::Up | KeyCode::Char('k') => Some(LiveAction::Command(Command::MoveUp)),
                    KeyCode::Down | KeyCode::Char('j') => {
                        Some(LiveAction::Command(Command::MoveDown))
                    }
                    _ => None,
                };
            }
            if in_form_dialog {
                return match key.code {
                    KeyCode::Esc => Some(LiveAction::Command(Command::FormCancel)),
                    KeyCode::Up => Some(LiveAction::Command(Command::FormMoveUp)),
                    KeyCode::Down => Some(LiveAction::Command(Command::FormMoveDown)),
                    KeyCode::Left => Some(LiveAction::Command(Command::FormSelectPrev)),
                    KeyCode::Right => Some(LiveAction::Command(Command::FormSelectNext)),
                    KeyCode::Backspace => Some(LiveAction::Command(Command::FormBackspace)),
                    KeyCode::Tab => Some(LiveAction::Command(Command::FormInsertNewline)),
                    KeyCode::Enter => Some(LiveAction::Command(Command::FormSubmit)),
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(LiveAction::Command(Command::FormInsertChar(ch)))
                    }
                    _ => None,
                };
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => Some(LiveAction::Command(Command::MoveUp)),
                KeyCode::Down | KeyCode::Char('j') => Some(LiveAction::Command(Command::MoveDown)),
                KeyCode::PageUp => Some(LiveAction::Command(Command::MovePageUp)),
                KeyCode::PageDown => Some(LiveAction::Command(Command::MovePageDown)),
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(LiveAction::Command(Command::MoveHalfPageUp))
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(LiveAction::Command(Command::MoveHalfPageDown))
                }
                KeyCode::Char('g') => Some(LiveAction::Command(Command::MoveTop)),
                KeyCode::Char('G') => Some(LiveAction::Command(Command::MoveBottom)),
                KeyCode::Char('W') => Some(LiveAction::SaveReplay),
                KeyCode::Esc | KeyCode::Char('q') => Some(LiveAction::Command(Command::Quit)),
                KeyCode::Char('a') => Some(LiveAction::Command(Command::StartInsert)),
                KeyCode::Char('e') => Some(LiveAction::Command(Command::StartEdit)),
                KeyCode::Char('d') => Some(LiveAction::Command(Command::DeleteCurrent)),
                KeyCode::Char('.') => Some(LiveAction::Command(Command::AcceptTrivialChain)),
                KeyCode::Char('v') => Some(LiveAction::Command(Command::ToggleVisual)),
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(LiveAction::Command(Command::Char(ch)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn map_event_to_replay_action(event: Event) -> Option<ReplayAction> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Left | KeyCode::Char('h') => Some(ReplayAction::Prev),
            KeyCode::Right | KeyCode::Char('l') => Some(ReplayAction::Next),
            KeyCode::Esc | KeyCode::Char('q') => Some(ReplayAction::Quit),
            _ => None,
        },
        _ => None,
    }
}

fn render<R: TraceRuntime>(
    frame: &mut ratatui::Frame<'_>,
    session: &mut EditorSession<
        crate::editor::TraceItem<
            R::SyncOp,
            R::AsyncOp,
            R::SyncResult,
            R::SyncError,
            R::AsyncResult,
        >,
    >,
    runtime: &R,
) {
    let snapshot = snapshot_for(&session.state, runtime).unwrap_or(ViewSnapshot {
        rows: Vec::new(),
        replay_error: Some("failed to render trace".to_string()),
        steps: Vec::new(),
        trivial_preview: Vec::new(),
    });
    render_trace_view(
        frame,
        session,
        &snapshot,
        "j/k: step move  PgUp/PgDn/C-u/C-d: scroll  zz: center  v: visual  a: add  e: edit  d: delete  .: trivial chain  W: save replay  esc/q: quit",
        None,
    );
}

fn render_replay<R: TraceRuntime>(
    frame: &mut ratatui::Frame<'_>,
    replay_path: &Path,
    state: &AppState<RuntimeTraceItem<R>>,
    commands: &[Command],
    state_index: usize,
    runtime: &R,
) {
    let mut session = EditorSession {
        path: replay_path.to_path_buf(),
        state: state.clone(),
    };
    let replay_title = if state_index == 0 {
        format!("state 0/{} initial", commands.len())
    } else if let Some(command) = commands.get(state_index - 1) {
        format!("state {state_index}/{} after {:?}", commands.len(), command)
    } else {
        format!("state {state_index}/{}", commands.len())
    };
    let snapshot = snapshot_for(&session.state, runtime).unwrap_or(ViewSnapshot {
        rows: Vec::new(),
        replay_error: Some("failed to render trace".to_string()),
        steps: Vec::new(),
        trivial_preview: Vec::new(),
    });
    render_trace_view(
        frame,
        &mut session,
        &snapshot,
        "j/k: step move  PgUp/PgDn/C-u/C-d: scroll  zz: center  v: visual  a: add  e: edit  d: delete  .: trivial chain  W: save replay  esc/q: quit",
        Some(replay_title.as_str()),
    );
}

pub fn render_state_to_text<R: TraceRuntime>(
    session: &mut EditorSession<
        crate::editor::TraceItem<
            R::SyncOp,
            R::AsyncOp,
            R::SyncResult,
            R::SyncError,
            R::AsyncResult,
        >,
    >,
    runtime: &R,
    width: u16,
    height: u16,
) -> Result<String, String> {
    session.state.view.terminal_width = width;
    session.state.view.terminal_height = height;
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).map_err(|err| err.to_string())?;
    terminal
        .draw(|frame| render(frame, session, runtime))
        .map_err(|err| err.to_string())?;
    let buffer = terminal.backend().buffer().clone();
    let area = *buffer.area();
    let mut lines = Vec::new();
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            line.push_str(buffer[(x, y)].symbol());
        }
        while line.ends_with(' ') {
            line.pop();
        }
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

pub fn render_state_to_ansi<R: TraceRuntime>(
    session: &mut EditorSession<
        crate::editor::TraceItem<
            R::SyncOp,
            R::AsyncOp,
            R::SyncResult,
            R::SyncError,
            R::AsyncResult,
        >,
    >,
    runtime: &R,
    width: u16,
    height: u16,
) -> Result<String, String> {
    session.state.view.terminal_width = width;
    session.state.view.terminal_height = height;
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).map_err(|err| err.to_string())?;
    terminal
        .draw(|frame| render(frame, session, runtime))
        .map_err(|err| err.to_string())?;
    let buffer = terminal.backend().buffer().clone();
    let area = *buffer.area();
    let mut output = String::new();
    for y in area.top()..area.bottom() {
        let mut current_style = None;
        for x in area.left()..area.right() {
            let cell = &buffer[(x, y)];
            if current_style != Some(cell_style(cell)) {
                output.push_str(&ansi_for_cell_style(cell));
                current_style = Some(cell_style(cell));
            }
            output.push_str(cell.symbol());
        }
        output.push_str("\x1b[0m");
        if y + 1 < area.bottom() {
            output.push('\n');
        }
    }
    Ok(output)
}

fn cell_style(cell: &Cell) -> (Color, Color, Modifier) {
    (cell.fg, cell.bg, cell.modifier)
}

fn ansi_for_cell_style(cell: &Cell) -> String {
    let (fg, bg, modifier) = cell_style(cell);
    ansi_for_style(fg, bg, modifier)
}

fn ansi_for_style(fg: Color, bg: Color, modifier: Modifier) -> String {
    let mut parts = vec!["0".to_string()];
    parts.extend(color_to_ansi(fg, false));
    parts.extend(color_to_ansi(bg, true));
    if modifier.contains(Modifier::BOLD) {
        parts.push("1".to_string());
    }
    if modifier.contains(Modifier::DIM) {
        parts.push("2".to_string());
    }
    if modifier.contains(Modifier::ITALIC) {
        parts.push("3".to_string());
    }
    if modifier.contains(Modifier::UNDERLINED) {
        parts.push("4".to_string());
    }
    if modifier.contains(Modifier::SLOW_BLINK) {
        parts.push("5".to_string());
    }
    if modifier.contains(Modifier::RAPID_BLINK) {
        parts.push("6".to_string());
    }
    if modifier.contains(Modifier::REVERSED) {
        parts.push("7".to_string());
    }
    if modifier.contains(Modifier::HIDDEN) {
        parts.push("8".to_string());
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        parts.push("9".to_string());
    }
    format!("\x1b[{}m", parts.join(";"))
}

fn color_to_ansi(color: Color, is_background: bool) -> Vec<String> {
    match color {
        Color::Reset => vec![(if is_background { 49 } else { 39 }).to_string()],
        Color::Black => vec![(if is_background { 40 } else { 30 }).to_string()],
        Color::Red => vec![(if is_background { 41 } else { 31 }).to_string()],
        Color::Green => vec![(if is_background { 42 } else { 32 }).to_string()],
        Color::Yellow => vec![(if is_background { 43 } else { 33 }).to_string()],
        Color::Blue => vec![(if is_background { 44 } else { 34 }).to_string()],
        Color::Magenta => vec![(if is_background { 45 } else { 35 }).to_string()],
        Color::Cyan => vec![(if is_background { 46 } else { 36 }).to_string()],
        Color::Gray => vec![(if is_background { 47 } else { 37 }).to_string()],
        Color::DarkGray => vec![(if is_background { 100 } else { 90 }).to_string()],
        Color::LightRed => vec![(if is_background { 101 } else { 91 }).to_string()],
        Color::LightGreen => vec![(if is_background { 102 } else { 92 }).to_string()],
        Color::LightYellow => vec![(if is_background { 103 } else { 93 }).to_string()],
        Color::LightBlue => vec![(if is_background { 104 } else { 94 }).to_string()],
        Color::LightMagenta => vec![(if is_background { 105 } else { 95 }).to_string()],
        Color::LightCyan => vec![(if is_background { 106 } else { 96 }).to_string()],
        Color::White => vec![(if is_background { 107 } else { 97 }).to_string()],
        Color::Rgb(r, g, b) => {
            let prefix = if is_background { 48 } else { 38 };
            vec![
                prefix.to_string(),
                "2".to_string(),
                r.to_string(),
                g.to_string(),
                b.to_string(),
            ]
        }
        Color::Indexed(index) => {
            let prefix = if is_background { 48 } else { 38 };
            vec![prefix.to_string(), "5".to_string(), index.to_string()]
        }
    }
}

fn save_replay_snapshot<R: TraceRuntime>(
    trace_path: &Path,
    recorder: &ReplayRecorder<RuntimeTraceItem<R>>,
    runtime: &R,
) -> Result<std::path::PathBuf, String> {
    let timestamp = OffsetDateTime::now_utc()
        .format(REPLAY_TIMESTAMP_FORMAT)
        .map_err(|err| err.to_string())?;
    let replay_path = trace_path.with_file_name(format!("{timestamp}.replay.json"));
    let replay = ReplayEnvelope::new(recorder.initial_state.clone(), recorder.commands.clone());
    save_runtime_replay(runtime, &replay_path, &replay)?;
    Ok(replay_path)
}

fn render_trace_view<T>(
    frame: &mut ratatui::Frame<'_>,
    session: &mut EditorSession<T>,
    snapshot: &ViewSnapshot,
    help_text: &str,
    title_right: Option<&str>,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(4)])
        .split(area);

    let selection_range = session
        .state
        .view
        .selection_anchor_step_index
        .map(|anchor| {
            (
                anchor.min(session.state.view.cursor_step_index),
                anchor.max(session.state.view.cursor_step_index),
            )
        });
    let gutter_width = snapshot
        .rows
        .iter()
        .chain(snapshot.trivial_preview.iter())
        .map(|row| row.timeline.chars().count())
        .max()
        .unwrap_or(0);
    let items = if snapshot.rows.is_empty() {
        vec![ListItem::new("(no visible events)")]
    } else {
        let mut items = Vec::new();
        for (step_index, step) in snapshot.steps.iter().enumerate() {
            for (row_offset, row) in snapshot.rows[step.start_row..=step.end_row]
                .iter()
                .enumerate()
            {
                let content = if gutter_width == 0 {
                    row.text.clone()
                } else {
                    format!(
                        "{:<width$} {}",
                        row.timeline,
                        row.text,
                        width = gutter_width
                    )
                };
                let mut style = Style::default();
                if row.is_invalid {
                    style = style.fg(Color::Red);
                }
                if selection_range
                    .as_ref()
                    .is_some_and(|(start, end)| *start <= step_index && step_index <= *end)
                {
                    style = style.bg(Color::DarkGray);
                }
                if step_index == session.state.view.cursor_step_index {
                    style = style.bg(Color::DarkGray);
                }
                if step_index == session.state.view.cursor_step_index && row_offset == 0 {
                    style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
                }
                items.push(ListItem::new(content).style(style));
            }
        }
        items.extend(snapshot.trivial_preview.iter().map(|preview| {
            let content = if gutter_width == 0 {
                preview.text.clone()
            } else {
                format!(
                    "{:<width$} {}",
                    preview.timeline,
                    preview.text,
                    width = gutter_width
                )
            };
            ListItem::new(content).style(Style::default().fg(Color::DarkGray))
        }));
        items
    };
    let mut state = ListState::default();
    if !snapshot.rows.is_empty() {
        let selected_row = snapshot
            .steps
            .get(session.state.view.cursor_step_index)
            .map(|step| step.start_row);
        state = state
            .with_selected(selected_row)
            .with_offset(session.state.view.scroll_offset);
    }
    let mut block = Block::default()
        .title(format!("Trace: {}", session.path.display()))
        .borders(Borders::ALL);
    if let Some(title_right) = title_right {
        block = block.title(Line::from(title_right.to_string()).alignment(Alignment::Right));
    }
    let widget = List::new(items).block(block);
    frame.render_stateful_widget(widget, chunks[0], &mut state);

    let mut help = vec![Line::from(help_text.to_string())];
    if let Some(replay_error) = &snapshot.replay_error {
        help.push(Line::from(format!("Replay error: {replay_error}")));
    }
    if let Some(status) = &session.state.view.status {
        help.push(Line::from(status.clone()));
    }
    frame.render_widget(
        Paragraph::new(help).block(Block::default().title("Status").borders(Borders::ALL)),
        chunks[1],
    );
    match &session.state.view.dialog {
        TraceViewDialog::None => {}
        TraceViewDialog::Choice {
            target,
            choices,
            selected,
        } => render_choice_dialog(frame, area, choice_dialog_title(target), choices, *selected),
        TraceViewDialog::Form {
            spec,
            state,
            selected_field,
            ..
        } => {
            let popup = centered_rect(70, 12, area);
            frame.render_widget(Clear, popup);
            render_form_dialog(frame, popup, spec, state, *selected_field);
        }
    }
}

fn choice_dialog_title(target: &crate::editor::DialogTarget) -> &'static str {
    match target {
        crate::editor::DialogTarget::InsertAfterStep { .. } => "Add inbound event",
        crate::editor::DialogTarget::EditInboundOfStep { .. } => "Edit inbound event",
    }
}

fn render_choice_dialog(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    title: &str,
    choices: &[crate::editor::InsertionChoice],
    selected: usize,
) {
    let popup = centered_rect(70, 12, area);
    frame.render_widget(Clear, popup);
    let items = choices
        .iter()
        .map(|choice| ListItem::new(choice.label.clone()))
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    if !choices.is_empty() {
        state.select(Some(selected));
    }
    let widget = List::new(items)
        .block(
            Block::default()
                .title(format!("{title} (enter: confirm  esc: cancel)"))
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(widget, popup, &mut state);
}

fn render_form_dialog(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    spec: &crate::editor::FormSpec,
    state: &crate::editor::FormState,
    selected_field: usize,
) {
    let mut lines = spec
        .details
        .iter()
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();
    if !lines.is_empty() {
        lines.push(Line::from(String::new()));
    }
    if spec.fields.is_empty() {
        lines.push(Line::from("No fields. Enter to save, esc to cancel."));
    } else {
        for (index, field) in spec.fields.iter().enumerate() {
            let prefix = if index == selected_field { ">" } else { " " };
            let value = match &field.kind {
                crate::editor::FormFieldKind::Text { .. } => match state.get(&field.id) {
                    Some(crate::editor::FormValue::Text(text)) => text.clone(),
                    _ => String::new(),
                },
                crate::editor::FormFieldKind::Select { options } => match state.get(&field.id) {
                    Some(crate::editor::FormValue::Select(selected)) => options
                        .get(*selected)
                        .cloned()
                        .unwrap_or_else(|| "<invalid>".to_string()),
                    _ => "<unset>".to_string(),
                },
                crate::editor::FormFieldKind::Toggle {
                    false_label,
                    true_label,
                } => match state.get(&field.id) {
                    Some(crate::editor::FormValue::Toggle(true)) => true_label.clone(),
                    Some(crate::editor::FormValue::Toggle(false)) => false_label.clone(),
                    _ => "<unset>".to_string(),
                },
            };
            lines.push(Line::from(format!("{prefix} {}: {value}", field.label)));
            if let Some(help) = &field.help {
                lines.push(Line::from(format!("    {help}")));
            }
        }
        lines.push(Line::from(String::new()));
        lines.push(Line::from(
            "up/down: field  left/right: change choice  text: edit  tab: newline  enter: save  esc: cancel",
        ));
    }
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(spec.title.as_str())
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_width = area.width.saturating_mul(percent_x).max(1) / 100;
    let popup_height = height.min(area.height.saturating_sub(2)).max(3);
    Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height) / 2,
        width: popup_width.max(10),
        height: popup_height,
    }
}
