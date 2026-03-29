use std::fs;
use std::path::{Path, PathBuf};

use super::{
    AppState, Command, CommandOutcome, DialogMode, FormController, FormResult, FormTarget,
    InsertionChoice, PromptKind, RenderedTrace, RunDocument, Screen, TraceEntry, TraceListState,
    TraceViewDialog, TraceViewState,
};

pub trait TraceRuntime {
    fn render_trace(&self, document: &RunDocument) -> Result<RenderedTrace, String>;
    fn preview_trivial_chain(
        &self,
        document: &RunDocument,
        insertion_index: usize,
    ) -> Result<Vec<String>, String>;
    fn apply_trivial_chain(
        &self,
        document: &mut RunDocument,
        insertion_index: usize,
    ) -> Result<usize, String>;
    fn insertion_choices(
        &self,
        document: &RunDocument,
        insertion_index: usize,
    ) -> Result<Vec<InsertionChoice>, String>;
    fn begin_insert_form(
        &self,
        document: &RunDocument,
        insertion_index: usize,
        choice_index: usize,
    ) -> Result<Box<dyn FormController>, String>;
    fn edit_choices(
        &self,
        document: &RunDocument,
        item_index: usize,
    ) -> Result<Vec<InsertionChoice>, String>;
    fn begin_edit_form(
        &self,
        document: &RunDocument,
        item_index: usize,
        choice_index: usize,
    ) -> Result<Box<dyn FormController>, String>;
    fn apply_form(
        &self,
        document: &mut RunDocument,
        target: &FormTarget,
        items: Vec<serde_json::Value>,
    ) -> Result<(), String>;
    fn delete_item(&self, document: &mut RunDocument, item_index: usize) -> Result<(), String>;
    fn delete_items(
        &self,
        document: &mut RunDocument,
        item_indices: Vec<usize>,
    ) -> Result<(), String>;
}

pub fn discover_traces(directory: &Path) -> Result<Vec<TraceEntry>, String> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(directory)
        .map_err(|err| format!("failed to read directory {}: {err}", directory.display()))?;

    for entry in read_dir {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let Ok(document) = load_document(&path) else {
            continue;
        };
        if !document.is_simulator_run() {
            continue;
        }

        let Some(file_name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
        else {
            continue;
        };

        entries.push(TraceEntry { path, file_name });
    }

    entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(entries)
}

pub fn load_document(path: &Path) -> Result<RunDocument, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let document: RunDocument = serde_json::from_str(&contents)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    if !document.is_simulator_run() {
        return Err(format!("{} is not a simulator run", path.display()));
    }
    Ok(document)
}

pub fn save_document(path: &Path, document: &RunDocument) -> Result<(), String> {
    let contents = serde_json::to_string_pretty(document)
        .map_err(|err| format!("failed to serialize {}: {err}", path.display()))?;
    fs::write(path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

pub fn create_trace(directory: &Path, file_name: &str) -> Result<PathBuf, String> {
    let file_name = normalize_trace_name(file_name)?;
    let path = directory.join(file_name);
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    save_document(&path, &RunDocument::default())?;
    Ok(path)
}

pub fn copy_trace(source: &Path, destination_name: &str) -> Result<PathBuf, String> {
    let destination_name = normalize_trace_name(destination_name)?;
    let destination = source
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", source.display()))?
        .join(destination_name);
    if destination.exists() {
        return Err(format!("{} already exists", destination.display()));
    }
    fs::copy(source, &destination).map_err(|err| {
        format!(
            "failed to copy {} to {}: {err}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(destination)
}

pub fn open_trace(runtime: &impl TraceRuntime, path: &Path) -> Result<TraceViewState, String> {
    let document = load_document(path)?;
    let rendered = runtime.render_trace(&document)?;
    let mut view = TraceViewState {
        path: path.to_path_buf(),
        document,
        rows: rendered.rows,
        cursor: 0,
        selection_anchor: None,
        scroll_offset: 0,
        viewport_height: 0,
        dialog: TraceViewDialog::None,
        status: None,
        replay_error: rendered.replay_error,
        trivial_preview: Vec::new(),
        pending_zz: false,
    };
    refresh_trivial_preview(&mut view, runtime);
    Ok(view)
}

fn selection_range(view: &TraceViewState) -> Option<(usize, usize)> {
    let anchor = view.selection_anchor?;
    let (anchor_start, anchor_end) = step_bounds(&view.rows, anchor);
    let (cursor_start, cursor_end) = step_bounds(&view.rows, view.cursor);
    Some((anchor_start.min(cursor_start), anchor_end.max(cursor_end)))
}

fn step_bounds(rows: &[super::VisibleRow], row_index: usize) -> (usize, usize) {
    if rows.is_empty() {
        return (0, 0);
    }
    let row_index = row_index.min(rows.len() - 1);
    let key = rows[row_index].insertion_index;
    let mut start = row_index;
    while start > 0 && rows[start - 1].insertion_index == key {
        start -= 1;
    }
    let mut end = row_index;
    while end + 1 < rows.len() && rows[end + 1].insertion_index == key {
        end += 1;
    }
    (start, end)
}

fn step_start(rows: &[super::VisibleRow], row_index: usize) -> usize {
    step_bounds(rows, row_index).0
}

fn step_end(rows: &[super::VisibleRow], row_index: usize) -> usize {
    step_bounds(rows, row_index).1
}

fn move_to_previous_step(view: &mut TraceViewState) {
    if view.rows.is_empty() {
        view.cursor = 0;
        return;
    }
    let start = step_start(&view.rows, view.cursor);
    if start == 0 {
        view.cursor = 0;
    } else {
        view.cursor = step_start(&view.rows, start - 1);
    }
    keep_cursor_visible(view.cursor, &mut view.scroll_offset, view.viewport_height);
}

fn move_to_next_step(view: &mut TraceViewState) {
    if view.rows.is_empty() {
        view.cursor = 0;
        return;
    }
    let end = step_end(&view.rows, view.cursor);
    if end + 1 < view.rows.len() {
        view.cursor = end + 1;
    }
    keep_cursor_visible(view.cursor, &mut view.scroll_offset, view.viewport_height);
}

fn move_cursor_by_page(view: &mut TraceViewState, delta_rows: isize) {
    if view.rows.is_empty() {
        view.cursor = 0;
        return;
    }
    let max = view.rows.len().saturating_sub(1) as isize;
    let target_row = (view.cursor as isize + delta_rows).clamp(0, max) as usize;
    view.cursor = step_start(&view.rows, target_row);
    keep_cursor_visible(view.cursor, &mut view.scroll_offset, view.viewport_height);
}

fn selected_script_item_indices(view: &TraceViewState) -> Vec<usize> {
    let rows: Box<dyn Iterator<Item = &super::VisibleRow> + '_> =
        if let Some((start, end)) = selection_range(view) {
            Box::new(view.rows[start..=end].iter())
        } else {
            Box::new(view.rows.get(view.cursor).into_iter())
        };

    let mut indices = rows
        .filter_map(|row| row.script_item_index)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();
    indices
}

enum CursorTarget {
    ScriptItemAtOrAfter(usize),
}

fn restore_cursor(next_view: &mut TraceViewState, target: CursorTarget) {
    let resolved = match target {
        CursorTarget::ScriptItemAtOrAfter(item_index) => next_view
            .rows
            .iter()
            .position(|row| {
                row.script_item_index
                    .is_some_and(|index| index >= item_index)
            })
            .or_else(|| {
                next_view
                    .rows
                    .iter()
                    .rposition(|row| row.script_item_index.is_some())
            })
            .unwrap_or(0),
    };
    next_view.cursor = resolved;
}

fn restore_viewport(
    next_view: &mut TraceViewState,
    previous_scroll_offset: usize,
    previous_viewport_height: usize,
) {
    next_view.viewport_height = previous_viewport_height;
    next_view.scroll_offset = previous_scroll_offset;
    keep_cursor_visible(
        next_view.cursor,
        &mut next_view.scroll_offset,
        next_view.viewport_height,
    );
}

fn preview_insertion_index(view: &TraceViewState) -> Option<usize> {
    let last_valid_row = view.rows.iter().rposition(|row| !row.is_invalid)?;
    if step_end(&view.rows, view.cursor) != last_valid_row {
        return None;
    }
    view.rows.get(last_valid_row).map(|row| row.insertion_index)
}

fn refresh_trivial_preview(view: &mut TraceViewState, runtime: &impl TraceRuntime) {
    view.trivial_preview = preview_insertion_index(view)
        .and_then(|insertion_index| {
            runtime
                .preview_trivial_chain(&view.document, insertion_index)
                .ok()
        })
        .unwrap_or_default();
}

pub fn refresh_trace_list(
    directory: &Path,
    selected_file_name: Option<&str>,
) -> Result<TraceListState, String> {
    let entries = discover_traces(directory)?;
    let selected = selected_file_name
        .and_then(|file_name| {
            entries
                .iter()
                .position(|entry| entry.file_name == file_name)
        })
        .unwrap_or(0)
        .min(entries.len().saturating_sub(1));

    Ok(TraceListState {
        directory: directory.to_path_buf(),
        entries,
        selected,
        scroll_offset: 0,
        viewport_height: 0,
        dialog: DialogMode::None,
        status: None,
    })
}

fn keep_cursor_visible(cursor: usize, scroll_offset: &mut usize, viewport_height: usize) {
    if viewport_height == 0 {
        return;
    }
    if cursor < *scroll_offset {
        *scroll_offset = cursor;
        return;
    }
    let last_visible = scroll_offset.saturating_add(viewport_height.saturating_sub(1));
    if cursor > last_visible {
        *scroll_offset = cursor.saturating_sub(viewport_height.saturating_sub(1));
    }
}

pub fn update(
    state: &mut AppState,
    command: Command,
    runtime: &impl TraceRuntime,
) -> CommandOutcome {
    match state.screen {
        Screen::TraceList(_) => update_trace_list(state, command, runtime),
        Screen::TraceView(_) => update_trace_view(state, command, runtime),
    }
}

fn update_trace_list(
    state: &mut AppState,
    command: Command,
    runtime: &impl TraceRuntime,
) -> CommandOutcome {
    let Screen::TraceList(list) = &mut state.screen else {
        unreachable!();
    };
    match (&mut list.dialog, command) {
        (DialogMode::Prompt { value, .. }, Command::PromptInsert(ch)) => {
            value.push(ch);
            return CommandOutcome::Noop;
        }
        (DialogMode::Prompt { value, .. }, Command::PromptBackspace) => {
            value.pop();
            return CommandOutcome::Noop;
        }
        (_, Command::PromptCancel) => {
            list.dialog = DialogMode::None;
            return CommandOutcome::Noop;
        }
        (DialogMode::Prompt { kind, value }, Command::PromptSubmit) => {
            let input = value.trim().to_string();
            let result = match kind {
                PromptKind::Create => create_trace(&list.directory, &input),
                PromptKind::CopySelected => {
                    let Some(entry) = list.entries.get(list.selected) else {
                        return CommandOutcome::Message("no trace selected to copy".to_string());
                    };
                    copy_trace(&entry.path, &input)
                }
            };
            list.dialog = DialogMode::None;
            match result {
                Ok(path) => match refresh_trace_list(
                    &list.directory,
                    path.file_name().and_then(|name| name.to_str()),
                ) {
                    Ok(next) => {
                        *list = next;
                        CommandOutcome::Message(format!("saved {}", path.display()))
                    }
                    Err(err) => CommandOutcome::Message(err),
                },
                Err(err) => CommandOutcome::Message(err),
            }
        }
        (DialogMode::Prompt { .. }, _) => CommandOutcome::Noop,
        (DialogMode::None, Command::MoveUp) => {
            if list.selected > 0 {
                list.selected -= 1;
                keep_cursor_visible(list.selected, &mut list.scroll_offset, list.viewport_height);
            }
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::MoveDown) => {
            if list.selected + 1 < list.entries.len() {
                list.selected += 1;
                keep_cursor_visible(list.selected, &mut list.scroll_offset, list.viewport_height);
            }
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::MoveTop) => {
            list.selected = 0;
            keep_cursor_visible(list.selected, &mut list.scroll_offset, list.viewport_height);
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::MoveBottom) => {
            list.selected = list.entries.len().saturating_sub(1);
            keep_cursor_visible(list.selected, &mut list.scroll_offset, list.viewport_height);
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::MovePageUp) => {
            let delta = list.viewport_height.max(1);
            list.selected = list.selected.saturating_sub(delta);
            keep_cursor_visible(list.selected, &mut list.scroll_offset, list.viewport_height);
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::MovePageDown) => {
            let delta = list.viewport_height.max(1);
            list.selected = (list.selected + delta).min(list.entries.len().saturating_sub(1));
            keep_cursor_visible(list.selected, &mut list.scroll_offset, list.viewport_height);
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::MoveHalfPageUp) => {
            let delta = (list.viewport_height.max(2) / 2).max(1);
            list.selected = list.selected.saturating_sub(delta);
            keep_cursor_visible(list.selected, &mut list.scroll_offset, list.viewport_height);
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::MoveHalfPageDown) => {
            let delta = (list.viewport_height.max(2) / 2).max(1);
            list.selected = (list.selected + delta).min(list.entries.len().saturating_sub(1));
            keep_cursor_visible(list.selected, &mut list.scroll_offset, list.viewport_height);
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::StartCreate) => {
            list.dialog = DialogMode::Prompt {
                kind: PromptKind::Create,
                value: String::new(),
            };
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::StartCopy) => {
            list.dialog = DialogMode::Prompt {
                kind: PromptKind::CopySelected,
                value: String::new(),
            };
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::OpenSelected) => {
            let Some(entry) = list.entries.get(list.selected) else {
                return CommandOutcome::Message("no trace selected".to_string());
            };
            match open_trace(runtime, &entry.path) {
                Ok(view) => {
                    state.screen = Screen::TraceView(view);
                    CommandOutcome::Noop
                }
                Err(err) => CommandOutcome::Message(err),
            }
        }
        (DialogMode::None, Command::Quit) => {
            state.should_quit = true;
            CommandOutcome::Noop
        }
        (DialogMode::None, Command::ClearStatus) => {
            list.status = None;
            CommandOutcome::Noop
        }
        (
            DialogMode::None,
            Command::DialogCancel
            | Command::DialogConfirm
            | Command::FormKey(_)
            | Command::StartInsert
            | Command::StartEdit
            | Command::DeleteCurrent
            | Command::AcceptTrivialChain
            | Command::ToggleVisual,
        ) => CommandOutcome::Noop,
        (DialogMode::None, _) => CommandOutcome::Noop,
    }
}

fn update_trace_view(
    state: &mut AppState,
    command: Command,
    runtime: &impl TraceRuntime,
) -> CommandOutcome {
    let Screen::TraceView(view) = &mut state.screen else {
        unreachable!();
    };

    match command {
        Command::ClearStatus => {
            view.pending_zz = true;
            return CommandOutcome::Noop;
        }
        Command::CenterCursor => {
            view.pending_zz = false;
        }
        _ => {
            view.pending_zz = false;
        }
    }

    match (&mut view.dialog, command.clone()) {
        (
            TraceViewDialog::Insert {
                selected,
                choices: _,
                ..
            },
            Command::MoveUp,
        ) => {
            if *selected > 0 {
                *selected -= 1;
            }
            return CommandOutcome::Noop;
        }
        (
            TraceViewDialog::Insert {
                selected, choices, ..
            },
            Command::MoveDown,
        ) => {
            if *selected + 1 < choices.len() {
                *selected += 1;
            }
            return CommandOutcome::Noop;
        }
        (
            TraceViewDialog::Edit {
                selected,
                choices: _,
                ..
            },
            Command::MoveUp,
        ) => {
            if *selected > 0 {
                *selected -= 1;
            }
            return CommandOutcome::Noop;
        }
        (
            TraceViewDialog::Edit {
                selected, choices, ..
            },
            Command::MoveDown,
        ) => {
            if *selected + 1 < choices.len() {
                *selected += 1;
            }
            return CommandOutcome::Noop;
        }
        (TraceViewDialog::Insert { .. }, Command::DialogCancel) => {
            view.dialog = TraceViewDialog::None;
            return CommandOutcome::Noop;
        }
        (TraceViewDialog::Edit { .. }, Command::DialogCancel) => {
            view.dialog = TraceViewDialog::None;
            return CommandOutcome::Noop;
        }
        (TraceViewDialog::Form { .. }, Command::DialogCancel) => {
            view.dialog = TraceViewDialog::None;
            return CommandOutcome::Noop;
        }
        (
            TraceViewDialog::Insert {
                insertion_index,
                selected,
                ..
            },
            Command::DialogConfirm,
        ) => {
            let insertion_index = *insertion_index;
            let selected = *selected;
            match runtime.begin_insert_form(&view.document, insertion_index, selected) {
                Ok(controller) => {
                    view.dialog = TraceViewDialog::Form {
                        target: FormTarget::Insert { insertion_index },
                        controller,
                    };
                    return CommandOutcome::Noop;
                }
                Err(err) => return CommandOutcome::Message(err),
            }
        }
        (
            TraceViewDialog::Edit {
                item_index,
                selected,
                ..
            },
            Command::DialogConfirm,
        ) => {
            let item_index = *item_index;
            let selected = *selected;
            match runtime.begin_edit_form(&view.document, item_index, selected) {
                Ok(controller) => {
                    view.dialog = TraceViewDialog::Form {
                        target: FormTarget::Edit { item_index },
                        controller,
                    };
                    return CommandOutcome::Noop;
                }
                Err(err) => return CommandOutcome::Message(err),
            }
        }
        (TraceViewDialog::Form { target, controller }, Command::FormKey(key)) => {
            match controller.handle_key(key) {
                Ok(FormResult::Continue) => return CommandOutcome::Noop,
                Ok(FormResult::Cancel) => {
                    view.dialog = TraceViewDialog::None;
                    return CommandOutcome::Noop;
                }
                Ok(FormResult::Save { items }) => {
                    let mut document = view.document.clone();
                    let previous_scroll_offset = view.scroll_offset;
                    let previous_viewport_height = view.viewport_height;
                    let cursor_target = match target {
                        FormTarget::Insert { insertion_index } => {
                            CursorTarget::ScriptItemAtOrAfter(*insertion_index)
                        }
                        FormTarget::Edit { item_index } => {
                            CursorTarget::ScriptItemAtOrAfter(*item_index)
                        }
                    };
                    match runtime.apply_form(&mut document, target, items) {
                        Ok(()) => {
                            let path = view.path.clone();
                            match save_document(&path, &document)
                                .and_then(|()| open_trace(runtime, &path))
                            {
                                Ok(mut next_view) => {
                                    restore_cursor(&mut next_view, cursor_target);
                                    restore_viewport(
                                        &mut next_view,
                                        previous_scroll_offset,
                                        previous_viewport_height,
                                    );
                                    refresh_trivial_preview(&mut next_view, runtime);
                                    next_view.status = Some(format!("saved {}", path.display()));
                                    state.screen = Screen::TraceView(next_view);
                                    return CommandOutcome::Noop;
                                }
                                Err(err) => return CommandOutcome::Message(err),
                            }
                        }
                        Err(err) => return CommandOutcome::Message(err),
                    }
                }
                Err(err) => return CommandOutcome::Message(err),
            }
        }
        (
            TraceViewDialog::Insert { .. }
            | TraceViewDialog::Edit { .. }
            | TraceViewDialog::Form { .. },
            _,
        ) => return CommandOutcome::Noop,
        (TraceViewDialog::None, _) => {}
    }

    match command {
        Command::MoveUp => {
            move_to_previous_step(view);
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::MoveDown => {
            move_to_next_step(view);
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::MoveTop => {
            view.cursor = 0;
            keep_cursor_visible(view.cursor, &mut view.scroll_offset, view.viewport_height);
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::MovePageUp => {
            move_cursor_by_page(view, -(view.viewport_height.max(1) as isize));
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::MovePageDown => {
            move_cursor_by_page(view, view.viewport_height.max(1) as isize);
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::MoveHalfPageUp => {
            move_cursor_by_page(view, -((view.viewport_height.max(2) / 2) as isize));
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::MoveHalfPageDown => {
            move_cursor_by_page(view, (view.viewport_height.max(2) / 2) as isize);
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::CenterCursor => {
            if view.viewport_height > 0 {
                view.scroll_offset = view.cursor.saturating_sub(view.viewport_height / 2);
            }
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::ToggleVisual => {
            view.selection_anchor = if view.selection_anchor.is_some() {
                None
            } else {
                Some(step_start(&view.rows, view.cursor))
            };
            CommandOutcome::Noop
        }
        Command::MoveBottom => {
            view.cursor = view.rows.len().saturating_sub(1);
            keep_cursor_visible(view.cursor, &mut view.scroll_offset, view.viewport_height);
            refresh_trivial_preview(view, runtime);
            CommandOutcome::Noop
        }
        Command::StartInsert => {
            let insertion_index = view
                .rows
                .get(view.cursor)
                .map(|row| row.insertion_index)
                .unwrap_or(0);
            match runtime.insertion_choices(&view.document, insertion_index) {
                Ok(choices) if choices.is_empty() => {
                    CommandOutcome::Message("no valid inbound events at this position".to_string())
                }
                Ok(choices) => {
                    view.dialog = TraceViewDialog::Insert {
                        insertion_index,
                        choices,
                        selected: 0,
                    };
                    CommandOutcome::Noop
                }
                Err(err) => CommandOutcome::Message(err),
            }
        }
        Command::StartEdit => {
            let Some(item_index) = view
                .rows
                .get(view.cursor)
                .and_then(|row| row.script_item_index)
            else {
                return CommandOutcome::Message("current row is not editable".to_string());
            };
            match runtime.edit_choices(&view.document, item_index) {
                Ok(choices) if choices.is_empty() => {
                    CommandOutcome::Message("no valid edits for this row".to_string())
                }
                Ok(choices) => {
                    view.dialog = TraceViewDialog::Edit {
                        item_index,
                        choices,
                        selected: 0,
                    };
                    CommandOutcome::Noop
                }
                Err(err) => CommandOutcome::Message(err),
            }
        }
        Command::DeleteCurrent => {
            let item_indices = selected_script_item_indices(view);
            if item_indices.is_empty() {
                return CommandOutcome::Message("current row is not deletable".to_string());
            }
            let first_item_index = item_indices[0];
            let mut document = view.document.clone();
            let previous_scroll_offset = view.scroll_offset;
            let previous_viewport_height = view.viewport_height;
            match runtime.delete_items(&mut document, item_indices) {
                Ok(()) => {
                    let path = view.path.clone();
                    match save_document(&path, &document).and_then(|()| open_trace(runtime, &path))
                    {
                        Ok(mut next_view) => {
                            restore_cursor(
                                &mut next_view,
                                CursorTarget::ScriptItemAtOrAfter(first_item_index),
                            );
                            restore_viewport(
                                &mut next_view,
                                previous_scroll_offset,
                                previous_viewport_height,
                            );
                            next_view.selection_anchor = None;
                            refresh_trivial_preview(&mut next_view, runtime);
                            next_view.status = Some(format!("saved {}", path.display()));
                            state.screen = Screen::TraceView(next_view);
                            CommandOutcome::Noop
                        }
                        Err(err) => CommandOutcome::Message(err),
                    }
                }
                Err(err) => CommandOutcome::Message(err),
            }
        }
        Command::AcceptTrivialChain => {
            let Some(insertion_index) = preview_insertion_index(view) else {
                return CommandOutcome::Message(
                    "trivial chain is only available at the end of the valid trace".to_string(),
                );
            };
            let mut document = view.document.clone();
            let previous_scroll_offset = view.scroll_offset;
            let previous_viewport_height = view.viewport_height;
            match runtime.apply_trivial_chain(&mut document, insertion_index) {
                Ok(inserted_count) if inserted_count == 0 => {
                    CommandOutcome::Message("no trivial events to apply".to_string())
                }
                Ok(inserted_count) => {
                    let path = view.path.clone();
                    match save_document(&path, &document).and_then(|()| open_trace(runtime, &path))
                    {
                        Ok(mut next_view) => {
                            restore_cursor(
                                &mut next_view,
                                CursorTarget::ScriptItemAtOrAfter(insertion_index),
                            );
                            restore_viewport(
                                &mut next_view,
                                previous_scroll_offset,
                                previous_viewport_height,
                            );
                            refresh_trivial_preview(&mut next_view, runtime);
                            next_view.status =
                                Some(format!("applied {inserted_count} trivial events"));
                            state.screen = Screen::TraceView(next_view);
                            CommandOutcome::Noop
                        }
                        Err(err) => CommandOutcome::Message(err),
                    }
                }
                Err(err) => CommandOutcome::Message(err),
            }
        }
        Command::Back => {
            let selected = view.path.file_name().and_then(|name| name.to_str());
            match refresh_trace_list(
                view.path.parent().unwrap_or_else(|| Path::new(".")),
                selected,
            ) {
                Ok(list) => {
                    state.screen = Screen::TraceList(list);
                    CommandOutcome::Noop
                }
                Err(err) => CommandOutcome::Message(err),
            }
        }
        Command::Quit => {
            state.should_quit = true;
            CommandOutcome::Noop
        }
        Command::ClearStatus => {
            view.status = None;
            CommandOutcome::Noop
        }
        Command::DialogCancel | Command::DialogConfirm => CommandOutcome::Noop,
        _ => CommandOutcome::Noop,
    }
}

fn normalize_trace_name(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("trace name cannot be empty".to_string());
    }
    if trimmed.contains('/') {
        return Err("trace name cannot contain '/'".to_string());
    }
    if trimmed.contains('\0') {
        return Err("trace name cannot contain NUL".to_string());
    }
    if trimmed.ends_with(".json") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed}.json"))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::editor::VisibleRow;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;

    struct TestRuntime;

    struct TestForm {
        replacement: serde_json::Value,
    }

    impl FormController for TestForm {
        fn render(&self, _frame: &mut ratatui::Frame<'_>, _area: Rect) {}

        fn handle_key(&mut self, key: KeyEvent) -> Result<FormResult, String> {
            match key.code {
                KeyCode::Enter => Ok(FormResult::Save {
                    items: vec![self.replacement.clone()],
                }),
                KeyCode::Esc => Ok(FormResult::Cancel),
                _ => Ok(FormResult::Continue),
            }
        }
    }

    impl TraceRuntime for TestRuntime {
        fn render_trace(&self, document: &RunDocument) -> Result<RenderedTrace, String> {
            let mut rows = Vec::new();
            rows.push(VisibleRow {
                timeline: String::new(),
                text: "CreateSync#0 BootReason".to_string(),
                insertion_index: 0,
                script_item_index: None,
                is_invalid: false,
            });
            for (index, item) in document.items.iter().enumerate() {
                rows.push(VisibleRow {
                    timeline: String::new(),
                    text: item.to_string(),
                    insertion_index: index + 1,
                    script_item_index: Some(index),
                    is_invalid: false,
                });
            }
            Ok(RenderedTrace {
                rows,
                replay_error: None,
            })
        }

        fn preview_trivial_chain(
            &self,
            _document: &RunDocument,
            _insertion_index: usize,
        ) -> Result<Vec<String>, String> {
            Ok(vec!["ReturnSync UnitOk".to_string()])
        }

        fn apply_trivial_chain(
            &self,
            document: &mut RunDocument,
            insertion_index: usize,
        ) -> Result<usize, String> {
            document.items.insert(
                insertion_index,
                serde_json::json!({"type":"return_sync","target":0,"trivial":true}),
            );
            Ok(1)
        }

        fn insertion_choices(
            &self,
            _document: &RunDocument,
            _insertion_index: usize,
        ) -> Result<Vec<InsertionChoice>, String> {
            Ok(vec![InsertionChoice {
                label: "ReturnSync#0 BootReason".to_string(),
            }])
        }

        fn begin_insert_form(
            &self,
            _document: &RunDocument,
            _insertion_index: usize,
            _choice_index: usize,
        ) -> Result<Box<dyn FormController>, String> {
            Ok(Box::new(TestForm {
                replacement: serde_json::json!({"type":"return_sync","target":0}),
            }))
        }

        fn edit_choices(
            &self,
            _document: &RunDocument,
            _item_index: usize,
        ) -> Result<Vec<InsertionChoice>, String> {
            Ok(vec![InsertionChoice {
                label: "EditedReturnSync".to_string(),
            }])
        }

        fn begin_edit_form(
            &self,
            _document: &RunDocument,
            _item_index: usize,
            _choice_index: usize,
        ) -> Result<Box<dyn FormController>, String> {
            Ok(Box::new(TestForm {
                replacement: serde_json::json!({"type":"return_sync","target":1}),
            }))
        }

        fn apply_form(
            &self,
            document: &mut RunDocument,
            target: &FormTarget,
            items: Vec<serde_json::Value>,
        ) -> Result<(), String> {
            match target {
                FormTarget::Insert { insertion_index } => {
                    for (offset, item) in items.into_iter().enumerate() {
                        document.items.insert(insertion_index + offset, item);
                    }
                }
                FormTarget::Edit { item_index } => {
                    document.items.splice(*item_index..=*item_index, items);
                }
            }
            Ok(())
        }

        fn delete_item(&self, document: &mut RunDocument, item_index: usize) -> Result<(), String> {
            document.items.remove(item_index);
            Ok(())
        }

        fn delete_items(
            &self,
            document: &mut RunDocument,
            mut item_indices: Vec<usize>,
        ) -> Result<(), String> {
            item_indices.sort_unstable();
            item_indices.dedup();
            for item_index in item_indices.into_iter().rev() {
                document.items.remove(item_index);
            }
            Ok(())
        }
    }

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("simulator-editor-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn discover_traces_filters_non_simulator_json() {
        let directory = temp_dir();
        fs::write(
            directory.join("notes.json"),
            "{\"kind\":\"other\",\"version\":1,\"items\":[]}",
        )
        .unwrap();
        save_document(&directory.join("trace.json"), &RunDocument::default()).unwrap();

        let entries = discover_traces(&directory).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name, "trace.json");
    }

    #[test]
    fn create_and_open_trace() {
        let directory = temp_dir();
        let path = create_trace(&directory, "sample").unwrap();
        let view = open_trace(&TestRuntime, &path).unwrap();

        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.rows[0].text, "CreateSync#0 BootReason");
    }

    #[test]
    fn insert_after_cursor_updates_document_and_resaves() {
        let directory = temp_dir();
        let path = create_trace(&directory, "sample").unwrap();
        let view = open_trace(&TestRuntime, &path).unwrap();
        let mut state = AppState {
            screen: Screen::TraceView(view),
            should_quit: false,
        };

        assert_eq!(
            update(&mut state, Command::StartInsert, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(&mut state, Command::DialogConfirm, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(
                &mut state,
                Command::FormKey(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                &TestRuntime,
            ),
            CommandOutcome::Noop
        );

        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.document.items.len(), 1);
        assert_eq!(view.cursor, 1);

        let saved = load_document(&path).unwrap();
        assert_eq!(saved.items.len(), 1);
    }

    #[test]
    fn edit_current_row_updates_document_and_resaves() {
        let directory = temp_dir();
        let path = create_trace(&directory, "sample").unwrap();
        save_document(
            &path,
            &RunDocument {
                items: vec![serde_json::json!({"type":"return_sync","target":0})],
                ..RunDocument::default()
            },
        )
        .unwrap();
        let view = open_trace(&TestRuntime, &path).unwrap();
        let mut state = AppState {
            screen: Screen::TraceView(view),
            should_quit: false,
        };
        let Screen::TraceView(view) = &mut state.screen else {
            panic!("expected trace view");
        };
        view.cursor = 1;

        assert_eq!(
            update(&mut state, Command::StartEdit, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(&mut state, Command::DialogConfirm, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(
                &mut state,
                Command::FormKey(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                &TestRuntime,
            ),
            CommandOutcome::Noop
        );

        let saved = load_document(&path).unwrap();
        assert_eq!(
            saved.items[0],
            serde_json::json!({"type":"return_sync","target":1})
        );
        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.cursor, 1);
    }

    #[test]
    fn delete_current_row_updates_document_and_resaves() {
        let directory = temp_dir();
        let path = create_trace(&directory, "sample").unwrap();
        save_document(
            &path,
            &RunDocument {
                items: vec![serde_json::json!({"type":"return_sync","target":0})],
                ..RunDocument::default()
            },
        )
        .unwrap();
        let view = open_trace(&TestRuntime, &path).unwrap();
        let mut state = AppState {
            screen: Screen::TraceView(view),
            should_quit: false,
        };
        let Screen::TraceView(view) = &mut state.screen else {
            panic!("expected trace view");
        };
        view.cursor = 1;

        assert_eq!(
            update(&mut state, Command::DeleteCurrent, &TestRuntime),
            CommandOutcome::Noop
        );

        let saved = load_document(&path).unwrap();
        assert!(saved.items.is_empty());
        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.cursor, 0);
    }

    #[test]
    fn moving_up_within_viewport_keeps_scroll_offset() {
        let mut state = AppState {
            screen: Screen::TraceView(TraceViewState {
                path: PathBuf::from("trace.json"),
                document: RunDocument::default(),
                rows: (0..20)
                    .map(|index| VisibleRow {
                        timeline: String::new(),
                        text: format!("row {index}"),
                        insertion_index: index,
                        script_item_index: Some(index),
                        is_invalid: false,
                    })
                    .collect(),
                cursor: 8,
                selection_anchor: None,
                scroll_offset: 5,
                viewport_height: 5,
                dialog: TraceViewDialog::None,
                status: None,
                replay_error: None,
                trivial_preview: Vec::new(),
                pending_zz: false,
            }),
            should_quit: false,
        };

        assert_eq!(
            update(&mut state, Command::MoveUp, &TestRuntime),
            CommandOutcome::Noop
        );

        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.cursor, 7);
        assert_eq!(view.scroll_offset, 5);
    }

    #[test]
    fn moving_up_past_top_of_viewport_scrolls_up() {
        let mut state = AppState {
            screen: Screen::TraceView(TraceViewState {
                path: PathBuf::from("trace.json"),
                document: RunDocument::default(),
                rows: (0..20)
                    .map(|index| VisibleRow {
                        timeline: String::new(),
                        text: format!("row {index}"),
                        insertion_index: index,
                        script_item_index: Some(index),
                        is_invalid: false,
                    })
                    .collect(),
                cursor: 5,
                selection_anchor: None,
                scroll_offset: 5,
                viewport_height: 5,
                dialog: TraceViewDialog::None,
                status: None,
                replay_error: None,
                trivial_preview: Vec::new(),
                pending_zz: false,
            }),
            should_quit: false,
        };

        assert_eq!(
            update(&mut state, Command::MoveUp, &TestRuntime),
            CommandOutcome::Noop
        );

        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.cursor, 4);
        assert_eq!(view.scroll_offset, 4);
    }

    #[test]
    fn insert_preserves_scroll_context() {
        let directory = temp_dir();
        let path = create_trace(&directory, "sample").unwrap();
        let view = open_trace(&TestRuntime, &path).unwrap();
        let mut state = AppState {
            screen: Screen::TraceView(view),
            should_quit: false,
        };
        let Screen::TraceView(view) = &mut state.screen else {
            panic!("expected trace view");
        };
        view.scroll_offset = 4;
        view.viewport_height = 5;

        assert_eq!(
            update(&mut state, Command::StartInsert, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(&mut state, Command::DialogConfirm, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(
                &mut state,
                Command::FormKey(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                &TestRuntime,
            ),
            CommandOutcome::Noop
        );

        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.viewport_height, 5);
        assert_eq!(view.scroll_offset, 1);
    }

    #[test]
    fn visual_delete_removes_selected_script_rows() {
        let directory = temp_dir();
        let path = create_trace(&directory, "sample").unwrap();
        save_document(
            &path,
            &RunDocument {
                items: vec![
                    serde_json::json!({"type":"a"}),
                    serde_json::json!({"type":"b"}),
                    serde_json::json!({"type":"c"}),
                ],
                ..RunDocument::default()
            },
        )
        .unwrap();
        let view = open_trace(&TestRuntime, &path).unwrap();
        let mut state = AppState {
            screen: Screen::TraceView(view),
            should_quit: false,
        };
        let Screen::TraceView(view) = &mut state.screen else {
            panic!("expected trace view");
        };
        view.cursor = 1;

        assert_eq!(
            update(&mut state, Command::ToggleVisual, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(&mut state, Command::MoveDown, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(&mut state, Command::DeleteCurrent, &TestRuntime),
            CommandOutcome::Noop
        );

        let saved = load_document(&path).unwrap();
        assert_eq!(saved.items, vec![serde_json::json!({"type":"c"})]);
    }

    #[test]
    fn page_down_moves_by_viewport_height() {
        let mut state = AppState {
            screen: Screen::TraceView(TraceViewState {
                path: PathBuf::from("trace.json"),
                document: RunDocument::default(),
                rows: (0..20)
                    .map(|index| VisibleRow {
                        timeline: String::new(),
                        text: format!("row {index}"),
                        insertion_index: index,
                        script_item_index: Some(index),
                        is_invalid: false,
                    })
                    .collect(),
                cursor: 0,
                selection_anchor: None,
                scroll_offset: 0,
                viewport_height: 5,
                dialog: TraceViewDialog::None,
                status: None,
                replay_error: None,
                trivial_preview: Vec::new(),
                pending_zz: false,
            }),
            should_quit: false,
        };

        assert_eq!(
            update(&mut state, Command::MovePageDown, &TestRuntime),
            CommandOutcome::Noop
        );
        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.cursor, 5);
    }

    #[test]
    fn stepwise_move_down_jumps_to_next_step_start() {
        let mut state = AppState {
            screen: Screen::TraceView(TraceViewState {
                path: PathBuf::from("trace.json"),
                document: RunDocument::default(),
                rows: vec![
                    VisibleRow {
                        timeline: String::new(),
                        text: "s0 a".into(),
                        insertion_index: 0,
                        script_item_index: None,
                        is_invalid: false,
                    },
                    VisibleRow {
                        timeline: String::new(),
                        text: "s0 b".into(),
                        insertion_index: 0,
                        script_item_index: None,
                        is_invalid: false,
                    },
                    VisibleRow {
                        timeline: String::new(),
                        text: "s1 a".into(),
                        insertion_index: 1,
                        script_item_index: Some(0),
                        is_invalid: false,
                    },
                    VisibleRow {
                        timeline: String::new(),
                        text: "s1 b".into(),
                        insertion_index: 1,
                        script_item_index: None,
                        is_invalid: false,
                    },
                    VisibleRow {
                        timeline: String::new(),
                        text: "s2 a".into(),
                        insertion_index: 2,
                        script_item_index: Some(1),
                        is_invalid: false,
                    },
                ],
                cursor: 0,
                selection_anchor: None,
                scroll_offset: 0,
                viewport_height: 5,
                dialog: TraceViewDialog::None,
                status: None,
                replay_error: None,
                trivial_preview: Vec::new(),
                pending_zz: false,
            }),
            should_quit: false,
        };

        assert_eq!(
            update(&mut state, Command::MoveDown, &TestRuntime),
            CommandOutcome::Noop
        );
        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.cursor, 2);
    }

    #[test]
    fn step_bounds_cover_whole_step() {
        let rows = vec![
            VisibleRow {
                timeline: String::new(),
                text: "a".into(),
                insertion_index: 0,
                script_item_index: None,
                is_invalid: false,
            },
            VisibleRow {
                timeline: String::new(),
                text: "b".into(),
                insertion_index: 0,
                script_item_index: None,
                is_invalid: false,
            },
            VisibleRow {
                timeline: String::new(),
                text: "c".into(),
                insertion_index: 1,
                script_item_index: Some(0),
                is_invalid: false,
            },
            VisibleRow {
                timeline: String::new(),
                text: "d".into(),
                insertion_index: 1,
                script_item_index: None,
                is_invalid: false,
            },
            VisibleRow {
                timeline: String::new(),
                text: "e".into(),
                insertion_index: 2,
                script_item_index: Some(1),
                is_invalid: false,
            },
        ];

        assert_eq!(step_bounds(&rows, 0), (0, 1));
        assert_eq!(step_bounds(&rows, 1), (0, 1));
        assert_eq!(step_bounds(&rows, 2), (2, 3));
        assert_eq!(step_bounds(&rows, 3), (2, 3));
    }

    #[test]
    fn zz_centers_cursor() {
        let mut state = AppState {
            screen: Screen::TraceView(TraceViewState {
                path: PathBuf::from("trace.json"),
                document: RunDocument::default(),
                rows: (0..20)
                    .map(|index| VisibleRow {
                        timeline: String::new(),
                        text: format!("row {index}"),
                        insertion_index: index,
                        script_item_index: Some(index),
                        is_invalid: false,
                    })
                    .collect(),
                cursor: 10,
                selection_anchor: None,
                scroll_offset: 0,
                viewport_height: 6,
                dialog: TraceViewDialog::None,
                status: None,
                replay_error: None,
                trivial_preview: Vec::new(),
                pending_zz: false,
            }),
            should_quit: false,
        };

        assert_eq!(
            update(&mut state, Command::ClearStatus, &TestRuntime),
            CommandOutcome::Noop
        );
        assert_eq!(
            update(&mut state, Command::CenterCursor, &TestRuntime),
            CommandOutcome::Noop
        );
        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.scroll_offset, 7);
    }

    #[test]
    fn accept_trivial_chain_inserts_without_form() {
        let directory = temp_dir();
        let path = create_trace(&directory, "sample").unwrap();
        let view = open_trace(&TestRuntime, &path).unwrap();
        let mut state = AppState {
            screen: Screen::TraceView(view),
            should_quit: false,
        };

        assert_eq!(
            update(&mut state, Command::AcceptTrivialChain, &TestRuntime),
            CommandOutcome::Noop
        );

        let saved = load_document(&path).unwrap();
        assert_eq!(saved.items.len(), 1);
        let Screen::TraceView(view) = &state.screen else {
            panic!("expected trace view");
        };
        assert_eq!(view.cursor, 1);
    }
}
