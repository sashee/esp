use std::path::PathBuf;

use crossterm::event::KeyEvent;

use super::{FormController, RunDocument};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceEntry {
    pub path: PathBuf,
    pub file_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceListState {
    pub directory: PathBuf,
    pub entries: Vec<TraceEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub viewport_height: usize,
    pub dialog: DialogMode,
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleRow {
    pub timeline: String,
    pub text: String,
    pub insertion_index: usize,
    pub script_item_index: Option<usize>,
    pub is_invalid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedTrace {
    pub rows: Vec<VisibleRow>,
    pub replay_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertionChoice {
    pub label: String,
}

pub enum FormTarget {
    Insert { insertion_index: usize },
    Edit { item_index: usize },
}

pub enum TraceViewDialog {
    None,
    Insert {
        insertion_index: usize,
        choices: Vec<InsertionChoice>,
        selected: usize,
    },
    Edit {
        item_index: usize,
        choices: Vec<InsertionChoice>,
        selected: usize,
    },
    Form {
        target: FormTarget,
        controller: Box<dyn FormController>,
    },
}

pub struct TraceViewState {
    pub path: PathBuf,
    pub document: RunDocument,
    pub rows: Vec<VisibleRow>,
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
    pub scroll_offset: usize,
    pub viewport_height: usize,
    pub dialog: TraceViewDialog,
    pub status: Option<String>,
    pub replay_error: Option<String>,
    pub trivial_preview: Vec<String>,
    pub pending_zz: bool,
}

pub enum Screen {
    TraceList(TraceListState),
    TraceView(TraceViewState),
}

pub struct AppState {
    pub screen: Screen,
    pub should_quit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptKind {
    Create,
    CopySelected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogMode {
    None,
    Prompt { kind: PromptKind, value: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    MoveUp,
    MoveDown,
    MoveTop,
    MoveBottom,
    MovePageUp,
    MovePageDown,
    MoveHalfPageUp,
    MoveHalfPageDown,
    CenterCursor,
    OpenSelected,
    StartInsert,
    StartEdit,
    DeleteCurrent,
    AcceptTrivialChain,
    ToggleVisual,
    Back,
    StartCreate,
    StartCopy,
    PromptInsert(char),
    PromptBackspace,
    PromptSubmit,
    PromptCancel,
    DialogConfirm,
    DialogCancel,
    FormKey(KeyEvent),
    ClearStatus,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    Noop,
    Message(String),
}
