use std::io;
use std::path::Path;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;

use crate::editor::{
    refresh_trace_list, update, AppState, Command, CommandOutcome, DialogMode, PromptKind, Screen,
    TraceRuntime, TraceViewDialog,
};

pub fn run_editor(runtime: &impl TraceRuntime, directory: &Path) -> Result<(), String> {
    let list = refresh_trace_list(directory, None)?;
    let mut state = AppState {
        screen: Screen::TraceList(list),
        should_quit: false,
    };

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide).map_err(|err| err.to_string())?;
    terminal::enable_raw_mode().map_err(|err| err.to_string())?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| err.to_string())?;

    let result = run_loop(&mut terminal, &mut state, runtime);

    let cleanup_result = (|| -> Result<(), String> {
        terminal::disable_raw_mode().map_err(|err| err.to_string())?;
        execute!(terminal.backend_mut(), Show, LeaveAlternateScreen)
            .map_err(|err| err.to_string())?;
        Ok(())
    })();

    result.and(cleanup_result)
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    runtime: &impl TraceRuntime,
) -> Result<(), String> {
    loop {
        terminal
            .draw(|frame| render(frame, state))
            .map_err(|err| err.to_string())?;
        if state.should_quit {
            return Ok(());
        }

        let event = event::read().map_err(|err| err.to_string())?;
        let Some(command) = map_event_to_command(state, event) else {
            continue;
        };

        let outcome = update(state, command, runtime);
        apply_outcome(state, outcome);
    }
}

fn apply_outcome(state: &mut AppState, outcome: CommandOutcome) {
    if let CommandOutcome::Message(message) = outcome {
        match &mut state.screen {
            Screen::TraceList(list) => list.status = Some(message),
            Screen::TraceView(view) => view.status = Some(message),
        }
    }
}

fn map_event_to_command(state: &AppState, event: Event) -> Option<Command> {
    let Event::Key(key) = event else {
        return None;
    };
    if key.kind != KeyEventKind::Press {
        return None;
    }

    let in_prompt = matches!(
        state.screen,
        Screen::TraceList(crate::editor::TraceListState {
            dialog: DialogMode::Prompt { .. },
            ..
        })
    );
    let in_choice_dialog = matches!(
        state.screen,
        Screen::TraceView(crate::editor::TraceViewState {
            dialog: TraceViewDialog::Insert { .. } | TraceViewDialog::Edit { .. },
            ..
        })
    );
    let in_form_dialog = matches!(
        state.screen,
        Screen::TraceView(crate::editor::TraceViewState {
            dialog: TraceViewDialog::Form { .. },
            ..
        })
    );

    if in_prompt {
        return match key.code {
            KeyCode::Esc => Some(Command::PromptCancel),
            KeyCode::Enter => Some(Command::PromptSubmit),
            KeyCode::Backspace => Some(Command::PromptBackspace),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Command::PromptInsert(ch))
            }
            _ => None,
        };
    }

    if in_choice_dialog {
        return match key.code {
            KeyCode::Esc => Some(Command::DialogCancel),
            KeyCode::Enter => Some(Command::DialogConfirm),
            KeyCode::Up | KeyCode::Char('k') => Some(Command::MoveUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Command::MoveDown),
            _ => None,
        };
    }

    if in_form_dialog {
        return Some(Command::FormKey(key));
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(Command::MoveUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Command::MoveDown),
        KeyCode::PageUp => Some(Command::MovePageUp),
        KeyCode::PageDown => Some(Command::MovePageDown),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Command::MoveHalfPageUp)
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Command::MoveHalfPageDown)
        }
        KeyCode::Char('g') => Some(Command::MoveTop),
        KeyCode::Char('G') => Some(Command::MoveBottom),
        KeyCode::Enter => Some(Command::OpenSelected),
        KeyCode::Esc => Some(Command::Back),
        KeyCode::Char('q') => Some(Command::Quit),
        KeyCode::Char('c') => Some(Command::StartCreate),
        KeyCode::Char('y') => Some(Command::StartCopy),
        KeyCode::Char('a') => Some(Command::StartInsert),
        KeyCode::Char('e') => Some(Command::StartEdit),
        KeyCode::Char('d') => Some(Command::DeleteCurrent),
        KeyCode::Char('.') => Some(Command::AcceptTrivialChain),
        KeyCode::Char('v') => Some(Command::ToggleVisual),
        KeyCode::Char('z') => match &state.screen {
            Screen::TraceView(view) if view.pending_zz => Some(Command::CenterCursor),
            Screen::TraceView(_) => Some(Command::ClearStatus),
            _ => None,
        },
        _ => None,
    }
}

fn render(frame: &mut ratatui::Frame<'_>, state: &mut AppState) {
    match &mut state.screen {
        Screen::TraceList(list) => render_trace_list(frame, list),
        Screen::TraceView(view) => render_trace_view(frame, view),
    }
}

fn render_trace_list(frame: &mut ratatui::Frame<'_>, list: &mut crate::editor::TraceListState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(4)])
        .split(area);

    let items = if list.entries.is_empty() {
        vec![ListItem::new("(no traces)")]
    } else {
        list.entries
            .iter()
            .map(|entry| ListItem::new(entry.file_name.clone()))
            .collect()
    };
    let mut state = ListState::default();
    if !list.entries.is_empty() {
        state = state
            .with_selected(Some(list.selected))
            .with_offset(list.scroll_offset);
    }
    list.viewport_height = chunks[0].height.saturating_sub(2) as usize;
    let widget = List::new(items)
        .block(
            Block::default()
                .title("Simulator Traces")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(widget, chunks[0], &mut state);

    let mut help = vec![Line::from(
        "j/k: move  enter: open  c: create  y: copy  q: quit",
    )];
    if let Some(status) = &list.status {
        help.push(Line::from(status.clone()));
    }
    frame.render_widget(
        Paragraph::new(help).block(Block::default().title("Status").borders(Borders::ALL)),
        chunks[1],
    );

    if let DialogMode::Prompt { kind, value } = &list.dialog {
        let area = centered_rect(60, 5, area);
        frame.render_widget(Clear, area);
        let title = match kind {
            PromptKind::Create => "New trace name",
            PromptKind::CopySelected => "Copy trace",
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(value.clone()),
                Line::from(""),
                Line::from("enter: confirm  esc: cancel"),
            ])
            .block(Block::default().title(title).borders(Borders::ALL)),
            area,
        );
    }
}

fn render_trace_view(frame: &mut ratatui::Frame<'_>, view: &mut crate::editor::TraceViewState) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(4)])
        .split(area);

    let selection_step_range = view.selection_anchor.map(|anchor| {
        let anchor_key = view
            .rows
            .get(anchor)
            .map(|row| row.insertion_index)
            .unwrap_or(0);
        let cursor_key = view
            .rows
            .get(view.cursor)
            .map(|row| row.insertion_index)
            .unwrap_or(anchor_key);
        (anchor_key.min(cursor_key), anchor_key.max(cursor_key))
    });
    let current_step_key = view.rows.get(view.cursor).map(|row| row.insertion_index);
    let items = if view.rows.is_empty() {
        vec![ListItem::new("(no visible events)")]
    } else {
        let gutter_width = view
            .rows
            .iter()
            .map(|row| row.timeline.chars().count())
            .max()
            .unwrap_or(0);
        let mut items = view
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
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
                if current_step_key.is_some_and(|key| key == row.insertion_index) {
                    style = style.bg(Color::DarkGray);
                }
                if selection_step_range.as_ref().is_some_and(|(start, end)| {
                    *start <= row.insertion_index && row.insertion_index <= *end
                }) {
                    style = style.bg(Color::DarkGray);
                }
                if index == view.cursor {
                    style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
                }
                ListItem::new(content).style(style)
            })
            .collect::<Vec<_>>();
        items.extend(view.trivial_preview.iter().map(|preview| {
            ListItem::new(format!("   {preview}")).style(Style::default().fg(Color::DarkGray))
        }));
        items
    };
    let mut state = ListState::default();
    if !view.rows.is_empty() {
        state = state
            .with_selected(Some(view.cursor))
            .with_offset(view.scroll_offset);
    }
    view.viewport_height = chunks[0].height.saturating_sub(2) as usize;
    let widget = List::new(items).block(
        Block::default()
            .title(format!("Trace: {}", view.path.display()))
            .borders(Borders::ALL),
    );
    frame.render_stateful_widget(widget, chunks[0], &mut state);

    let mut help = vec![Line::from(
        "j/k: step move  PgUp/PgDn/C-u/C-d: scroll  zz: center  v: visual  a: add  e: edit  d: delete  .: trivial chain  esc: back  q: quit",
    )];
    if let Some(replay_error) = &view.replay_error {
        help.push(Line::from(format!("Replay error: {replay_error}")));
    }
    if let Some(status) = &view.status {
        help.push(Line::from(status.clone()));
    }
    frame.render_widget(
        Paragraph::new(help).block(Block::default().title("Status").borders(Borders::ALL)),
        chunks[1],
    );

    match &view.dialog {
        TraceViewDialog::None => {}
        TraceViewDialog::Insert {
            choices, selected, ..
        } => {
            render_choice_dialog(frame, area, "Add inbound event", choices, *selected);
        }
        TraceViewDialog::Edit {
            choices, selected, ..
        } => {
            render_choice_dialog(frame, area, "Edit inbound event", choices, *selected);
        }
        TraceViewDialog::Form { controller, .. } => {
            let popup = centered_rect(70, 12, area);
            frame.render_widget(Clear, popup);
            controller.render(frame, popup);
        }
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
