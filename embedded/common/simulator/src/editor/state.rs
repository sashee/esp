use serde::{Deserialize, Serialize};

use super::{FormSpec, FormState};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleRow {
    pub timeline: String,
    pub text: String,
    pub insertion_index: usize,
    pub script_item_index: Option<usize>,
    pub is_invalid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedTrace {
    pub rows: Vec<VisibleRow>,
    pub replay_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsertionChoice {
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogTarget {
    InsertAfterStep { step_index: usize },
    EditInboundOfStep { step_index: usize },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeTarget {
    Insert { insertion_index: usize },
    Edit { item_index: usize },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceViewDialog {
    None,
    Choice {
        target: DialogTarget,
        choices: Vec<InsertionChoice>,
        selected: usize,
    },
    Form {
        target: DialogTarget,
        choice_index: usize,
        spec: FormSpec,
        state: FormState,
        selected_field: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceViewState<T> {
    pub trace: Vec<T>,
    pub cursor_step_index: usize,
    pub selection_anchor_step_index: Option<usize>,
    pub scroll_offset: usize,
    pub dialog: TraceViewDialog,
    pub status: Option<String>,
    pub last_char: Option<char>,
    pub terminal_width: u16,
    pub terminal_height: u16,
}

impl<T> Default for TraceViewState<T> {
    fn default() -> Self {
        Self {
            trace: Vec::new(),
            cursor_step_index: 0,
            selection_anchor_step_index: None,
            scroll_offset: 0,
            dialog: TraceViewDialog::None,
            status: None,
            last_char: None,
            terminal_width: 0,
            terminal_height: 0,
        }
    }
}

impl<T> TraceViewState<T> {
    pub fn new(trace: Vec<T>, terminal_width: u16, terminal_height: u16) -> Self {
        Self {
            trace,
            cursor_step_index: 0,
            selection_anchor_step_index: None,
            scroll_offset: 0,
            dialog: TraceViewDialog::None,
            status: None,
            last_char: None,
            terminal_width,
            terminal_height,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState<T> {
    pub view: TraceViewState<T>,
}

impl<T> AppState<T> {
    pub fn new(trace: Vec<T>, terminal_width: u16, terminal_height: u16) -> Self {
        Self {
            view: TraceViewState::new(trace, terminal_width, terminal_height),
        }
    }
}

impl<T> Default for AppState<T> {
    fn default() -> Self {
        Self {
            view: TraceViewState::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Resize { width: u16, height: u16 },
    MoveUp,
    MoveDown,
    MoveTop,
    MoveBottom,
    MovePageUp,
    MovePageDown,
    MoveHalfPageUp,
    MoveHalfPageDown,
    CenterCursor,
    StartInsert,
    StartEdit,
    DeleteCurrent,
    AcceptTrivialChain,
    ToggleVisual,
    Char(char),
    DialogConfirm,
    DialogCancel,
    FormCancel,
    FormMoveUp,
    FormMoveDown,
    FormSelectPrev,
    FormSelectNext,
    FormBackspace,
    FormInsertChar(char),
    FormInsertNewline,
    FormSubmit,
    ClearStatus,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect<T> {
    SaveTrace { trace: Vec<T> },
    Quit,
}
