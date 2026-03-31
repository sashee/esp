use super::replay::*;
use super::saved::*;
use super::*;

const FIELD_BOOT_REASON: &str = "boot_reason";
const FIELD_TICKS: &str = "ticks";
const FIELD_TEXT: &str = "text";
const FIELD_CONFIRM: &str = "confirm";

pub(super) fn form_spec_for_items(title: &str, items: &[SavedItem]) -> simulator::editor::FormSpec {
    match items {
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::BootReason,
            ..
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::BootReason { .. },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            result: SavedSyncResult::BootReason { .. },
            ..
        }] => simulator::editor::FormSpec {
            title: title.to_string(),
            details: Vec::new(),
            fields: vec![simulator::editor::FormField {
                id: FIELD_BOOT_REASON.to_string(),
                label: "Boot reason".to_string(),
                kind: simulator::editor::FormFieldKind::Select {
                    options: BOOT_REASON_OPTIONS
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                },
                help: None,
            }],
            auto_accept_if_complete: false,
        },
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::Now,
            ..
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::Now { .. },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            result: SavedSyncResult::Now { .. },
            ..
        }] => simulator::editor::FormSpec {
            title: title.to_string(),
            details: vec!["Ticks since boot.".to_string()],
            fields: vec![simulator::editor::FormField {
                id: FIELD_TICKS.to_string(),
                label: "Ticks".to_string(),
                kind: simulator::editor::FormFieldKind::Text { multiline: false },
                help: None,
            }],
            auto_accept_if_complete: true,
        },
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::StoreRead { .. },
            ..
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::StoreReadOk { .. },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            result: SavedSyncResult::StoreReadOk { .. },
            ..
        }] => {
            let (namespace, keys) = store_read_context(items);
            simulator::editor::FormSpec {
                title: title.to_string(),
                details: vec![
                    format!("namespace: {namespace}"),
                    format!("keys: {}", keys.join(", ")),
                ],
                fields: vec![simulator::editor::FormField {
                    id: FIELD_TEXT.to_string(),
                    label: "Payload".to_string(),
                    kind: simulator::editor::FormFieldKind::Text { multiline: true },
                    help: Some("Success values as key=value lines.".to_string()),
                }],
                auto_accept_if_complete: false,
            }
        }
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::StoreRead { .. },
            ..
        }, SavedItem::InboundErrorSync {
            error: SavedSyncError::StoreReadErr { .. },
            ..
        }]
        | [SavedItem::InboundErrorSync {
            error: SavedSyncError::StoreReadErr { .. },
            ..
        }] => {
            let (namespace, keys) = store_read_context(items);
            simulator::editor::FormSpec {
                title: title.to_string(),
                details: vec![
                    format!("namespace: {namespace}"),
                    format!("keys: {}", keys.join(", ")),
                ],
                fields: vec![simulator::editor::FormField {
                    id: FIELD_TEXT.to_string(),
                    label: "Message".to_string(),
                    kind: simulator::editor::FormFieldKind::Text { multiline: true },
                    help: Some("Error message.".to_string()),
                }],
                auto_accept_if_complete: false,
            }
        }
        [SavedItem::OutboundCreateSync { op, .. }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::UnitOk,
            ..
        }] if matches!(
            op,
            SavedSyncOp::StoreWrite { .. }
                | SavedSyncOp::StoreRemove { .. }
                | SavedSyncOp::TftSetDcLow
                | SavedSyncOp::TftSetDcHigh
                | SavedSyncOp::TftSetRstLow
                | SavedSyncOp::TftSetRstHigh
                | SavedSyncOp::TftWrite { .. }
        ) =>
        {
            simulator::editor::FormSpec {
                title: title.to_string(),
                details: vec![format!("operation: {}", format_saved_sync_op(op))],
                fields: Vec::new(),
                auto_accept_if_complete: true,
            }
        }
        [SavedItem::OutboundCreateSync { op, .. }, SavedItem::InboundErrorSync {
            error: SavedSyncError::UnitErr { .. },
            ..
        }] if matches!(
            op,
            SavedSyncOp::StoreWrite { .. }
                | SavedSyncOp::StoreRemove { .. }
                | SavedSyncOp::TftSetDcLow
                | SavedSyncOp::TftSetDcHigh
                | SavedSyncOp::TftSetRstLow
                | SavedSyncOp::TftSetRstHigh
                | SavedSyncOp::TftWrite { .. }
        ) =>
        {
            simulator::editor::FormSpec {
                title: title.to_string(),
                details: vec![format!("operation: {}", format_saved_sync_op(op))],
                fields: vec![simulator::editor::FormField {
                    id: FIELD_TEXT.to_string(),
                    label: "Message".to_string(),
                    kind: simulator::editor::FormFieldKind::Text { multiline: true },
                    help: Some("Error message.".to_string()),
                }],
                auto_accept_if_complete: false,
            }
        }
        [SavedItem::OutboundCreateAsync { .. }, SavedItem::InboundAbortAsync { .. }]
        | [SavedItem::OutboundCreateAsync { .. }, SavedItem::InboundCancelAsync { .. }]
        | [SavedItem::InboundAbortAsync { .. }]
        | [SavedItem::InboundCancelAsync { .. }] => simulator::editor::FormSpec {
            title: title.to_string(),
            details: items.iter().map(format_saved_item).collect(),
            fields: vec![simulator::editor::FormField {
                id: FIELD_CONFIRM.to_string(),
                label: "Confirm".to_string(),
                kind: simulator::editor::FormFieldKind::Toggle {
                    false_label: "no".to_string(),
                    true_label: "yes".to_string(),
                },
                help: Some("Toggle to confirm this non-trivial action.".to_string()),
            }],
            auto_accept_if_complete: false,
        },
        _ => simulator::editor::FormSpec {
            title: title.to_string(),
            details: items.iter().map(format_saved_item).collect(),
            fields: Vec::new(),
            auto_accept_if_complete: true,
        },
    }
}

pub(super) fn default_form_state_for_items(items: &[SavedItem]) -> simulator::editor::FormState {
    match items {
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::BootReason,
            ..
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::BootReason { value },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            result: SavedSyncResult::BootReason { value },
            ..
        }] => std::iter::once((
            FIELD_BOOT_REASON.to_string(),
            simulator::editor::FormValue::Select(
                BOOT_REASON_OPTIONS
                    .iter()
                    .position(|option| *option == value)
                    .unwrap_or(0),
            ),
        ))
        .collect(),
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::Now,
            ..
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::Now { ticks },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            result: SavedSyncResult::Now { ticks },
            ..
        }] => std::iter::once((
            FIELD_TICKS.to_string(),
            simulator::editor::FormValue::Text(ticks.to_string()),
        ))
        .collect(),
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::StoreRead { .. },
            ..
        }, SavedItem::InboundReturnSync { result, .. }]
        | [SavedItem::InboundReturnSync { result, .. }] => match result {
            SavedSyncResult::StoreReadOk { values } => [(
                FIELD_TEXT.to_string(),
                simulator::editor::FormValue::Text(format_store_read_text(values)),
            )]
            .into_iter()
            .collect(),
            SavedSyncResult::UnitOk => simulator::editor::FormState::new(),
            _ => simulator::editor::FormState::new(),
        },
        [SavedItem::OutboundCreateSync { .. }, SavedItem::InboundErrorSync { .. }]
        | [SavedItem::InboundErrorSync { .. }] => simulator::editor::FormState::new(),
        [SavedItem::OutboundCreateAsync { .. }, SavedItem::InboundAbortAsync { .. }]
        | [SavedItem::OutboundCreateAsync { .. }, SavedItem::InboundCancelAsync { .. }]
        | [SavedItem::InboundAbortAsync { .. }]
        | [SavedItem::InboundCancelAsync { .. }] => simulator::editor::FormState::new(),
        _ => simulator::editor::FormState::new(),
    }
}

pub(super) fn current_boot_reason_selection(document: &[SavedItem], item_index: usize) -> usize {
    parse_items(document)
        .ok()
        .and_then(|items| items.get(item_index).cloned())
        .and_then(|item| match item {
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::BootReason { value },
                ..
            } => BOOT_REASON_OPTIONS
                .iter()
                .position(|option| *option == value),
            _ => None,
        })
        .unwrap_or(0)
}

pub(super) fn current_store_read_text(document: &[SavedItem], item_index: usize) -> String {
    parse_items(document)
        .ok()
        .and_then(|items| items.get(item_index).cloned())
        .and_then(|item| match item {
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::StoreReadOk { values },
                ..
            } => Some(format_store_read_text(&values)),
            SavedItem::InboundErrorSync {
                error: SavedSyncError::StoreReadErr { message },
                ..
            } => Some(message),
            _ => None,
        })
        .unwrap_or_default()
}

pub(super) fn current_store_unit_text(document: &[SavedItem], item_index: usize) -> String {
    parse_items(document)
        .ok()
        .and_then(|items| items.get(item_index).cloned())
        .and_then(|item| match item {
            SavedItem::InboundErrorSync {
                error: SavedSyncError::UnitErr { message },
                ..
            } => Some(message),
            _ => None,
        })
        .unwrap_or_default()
}

pub(super) fn current_now_ticks(document: &[SavedItem], item_index: usize) -> Option<u64> {
    parse_items(document)
        .ok()
        .and_then(|items| items.get(item_index).cloned())
        .and_then(|item| match item {
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::Now { ticks },
                ..
            } => Some(ticks),
            _ => None,
        })
}

pub(super) fn edited_form_state_for_items(
    document: &[SavedItem],
    item_index: usize,
    items: &[SavedItem],
) -> simulator::editor::FormState {
    let mut state = default_form_state_for_items(items);
    match items {
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::BootReason,
            ..
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::BootReason { .. },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            result: SavedSyncResult::BootReason { .. },
            ..
        }] => {
            state.insert(
                FIELD_BOOT_REASON.to_string(),
                simulator::editor::FormValue::Select(current_boot_reason_selection(
                    document, item_index,
                )),
            );
        }
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::Now,
            ..
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::Now { .. },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            result: SavedSyncResult::Now { .. },
            ..
        }] => {
            if let Some(ticks) = current_now_ticks(document, item_index) {
                state.insert(
                    FIELD_TICKS.to_string(),
                    simulator::editor::FormValue::Text(ticks.to_string()),
                );
            }
        }
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::StoreRead { .. },
            ..
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::StoreReadOk { .. },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            result: SavedSyncResult::StoreReadOk { .. },
            ..
        }] => {
            state.insert(
                FIELD_TEXT.to_string(),
                simulator::editor::FormValue::Text(current_store_read_text(document, item_index)),
            );
        }
        [SavedItem::OutboundCreateSync {
            op: SavedSyncOp::StoreRead { .. },
            ..
        }, SavedItem::InboundErrorSync {
            error: SavedSyncError::StoreReadErr { .. },
            ..
        }]
        | [SavedItem::InboundErrorSync {
            error: SavedSyncError::StoreReadErr { .. },
            ..
        }] => {
            state.insert(
                FIELD_TEXT.to_string(),
                simulator::editor::FormValue::Text(current_store_read_text(document, item_index)),
            );
        }
        [SavedItem::OutboundCreateSync { op, .. }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::UnitOk,
            ..
        }] if matches!(
            op,
            SavedSyncOp::StoreWrite { .. }
                | SavedSyncOp::StoreRemove { .. }
                | SavedSyncOp::TftSetDcLow
                | SavedSyncOp::TftSetDcHigh
                | SavedSyncOp::TftSetRstLow
                | SavedSyncOp::TftSetRstHigh
                | SavedSyncOp::TftWrite { .. }
        ) => {}
        [SavedItem::OutboundCreateSync { op, .. }, SavedItem::InboundErrorSync {
            error: SavedSyncError::UnitErr { .. },
            ..
        }] if matches!(
            op,
            SavedSyncOp::StoreWrite { .. }
                | SavedSyncOp::StoreRemove { .. }
                | SavedSyncOp::TftSetDcLow
                | SavedSyncOp::TftSetDcHigh
                | SavedSyncOp::TftSetRstLow
                | SavedSyncOp::TftSetRstHigh
                | SavedSyncOp::TftWrite { .. }
        ) =>
        {
            state.insert(
                FIELD_TEXT.to_string(),
                simulator::editor::FormValue::Text(current_store_unit_text(document, item_index)),
            );
        }
        _ => {}
    }
    state
}

pub(super) fn format_store_read_text(values: &BTreeMap<String, String>) -> String {
    values
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn parse_store_read_text(
    allowed_keys: &[String],
    text: &str,
) -> Result<BTreeMap<String, String>, String> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("invalid store entry '{line}', expected key=value"));
        };
        let key = key.trim().to_string();
        if !allowed_keys.iter().any(|allowed| allowed == &key) {
            return Err(format!("unexpected store key '{key}'"));
        }
        values.insert(key, value.trim().to_string());
    }
    Ok(values)
}

fn store_read_context(items: &[SavedItem]) -> (String, Vec<String>) {
    match items.first() {
        Some(SavedItem::OutboundCreateSync {
            op: SavedSyncOp::StoreRead { namespace, keys },
            ..
        }) => (namespace.clone(), keys.clone()),
        _ => (String::new(), Vec::new()),
    }
}

pub(super) fn encode_items_from_form_state(
    items: &[SavedItem],
    state: &simulator::editor::FormState,
) -> Result<Vec<SavedItem>, String> {
    let output = match items {
        [SavedItem::OutboundCreateSync {
            id,
            op: SavedSyncOp::BootReason,
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::BootReason { .. },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            target: id,
            result: SavedSyncResult::BootReason { .. },
        }] => {
            let selected = match state.get(FIELD_BOOT_REASON) {
                Some(simulator::editor::FormValue::Select(selected)) => *selected,
                _ => 0,
            };
            let mut output = Vec::new();
            if matches!(items.first(), Some(SavedItem::OutboundCreateSync { .. })) {
                output.push(items[0].clone());
            }
            output.push(SavedItem::InboundReturnSync {
                target: id.clone(),
                result: SavedSyncResult::BootReason {
                    value: BOOT_REASON_OPTIONS
                        [selected.min(BOOT_REASON_OPTIONS.len().saturating_sub(1))]
                    .to_string(),
                },
            });
            output
        }
        [SavedItem::OutboundCreateSync {
            id,
            op: SavedSyncOp::Now,
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::Now { .. },
            ..
        }]
        | [SavedItem::InboundReturnSync {
            target: id,
            result: SavedSyncResult::Now { .. },
        }] => {
            let ticks = match state.get(FIELD_TICKS) {
                Some(simulator::editor::FormValue::Text(text)) => {
                    text.trim().parse::<u64>().map_err(|err| err.to_string())?
                }
                _ => return Err("missing ticks".to_string()),
            };
            let mut output = Vec::new();
            if matches!(items.first(), Some(SavedItem::OutboundCreateSync { .. })) {
                output.push(items[0].clone());
            }
            output.push(SavedItem::InboundReturnSync {
                target: id.clone(),
                result: SavedSyncResult::Now { ticks },
            });
            output
        }
        [SavedItem::OutboundCreateSync {
            id,
            op: SavedSyncOp::StoreRead { .. },
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::StoreReadOk { .. },
            ..
        }] => {
            let text = match state.get(FIELD_TEXT) {
                Some(simulator::editor::FormValue::Text(text)) => text.clone(),
                _ => String::new(),
            };
            let (_, keys) = store_read_context(items);
            let mut output = Vec::new();
            if matches!(items.first(), Some(SavedItem::OutboundCreateSync { .. })) {
                output.push(items[0].clone());
            }
            output.push(SavedItem::InboundReturnSync {
                target: id.clone(),
                result: SavedSyncResult::StoreReadOk {
                    values: parse_store_read_text(&keys, &text)?,
                },
            });
            output
        }
        [SavedItem::InboundErrorSync {
            target: id,
            error: SavedSyncError::StoreReadErr { .. },
        }] => {
            let text = match state.get(FIELD_TEXT) {
                Some(simulator::editor::FormValue::Text(text)) => text.clone(),
                _ => String::new(),
            };
            output_store_read_error_without_context(id, text)
        }
        [SavedItem::OutboundCreateSync {
            id,
            op: SavedSyncOp::StoreRead { .. },
        }, SavedItem::InboundErrorSync {
            error: SavedSyncError::StoreReadErr { .. },
            ..
        }] => {
            let text = match state.get(FIELD_TEXT) {
                Some(simulator::editor::FormValue::Text(text)) => text.clone(),
                _ => String::new(),
            };
            let mut output = Vec::new();
            output.push(items[0].clone());
            output.push(SavedItem::InboundErrorSync {
                target: id.clone(),
                error: SavedSyncError::StoreReadErr {
                    message: text.trim().to_string(),
                },
            });
            output
        }
        [SavedItem::OutboundCreateSync { id, op }, SavedItem::InboundReturnSync { .. }]
            if matches!(
                op,
                SavedSyncOp::StoreWrite { .. }
                    | SavedSyncOp::StoreRemove { .. }
                    | SavedSyncOp::TftSetDcLow
                    | SavedSyncOp::TftSetDcHigh
                    | SavedSyncOp::TftSetRstLow
                    | SavedSyncOp::TftSetRstHigh
                    | SavedSyncOp::TftWrite { .. }
            ) =>
        {
            let mut output = Vec::new();
            output.push(items[0].clone());
            output.push(SavedItem::InboundReturnSync {
                target: id.clone(),
                result: SavedSyncResult::UnitOk,
            });
            output
        }
        [SavedItem::OutboundCreateSync { id, op }, SavedItem::InboundErrorSync { .. }]
            if matches!(
                op,
                SavedSyncOp::StoreWrite { .. }
                    | SavedSyncOp::StoreRemove { .. }
                    | SavedSyncOp::TftSetDcLow
                    | SavedSyncOp::TftSetDcHigh
                    | SavedSyncOp::TftSetRstLow
                    | SavedSyncOp::TftSetRstHigh
                    | SavedSyncOp::TftWrite { .. }
            ) =>
        {
            let text = match state.get(FIELD_TEXT) {
                Some(simulator::editor::FormValue::Text(text)) => text.clone(),
                _ => String::new(),
            };
            let mut output = Vec::new();
            output.push(items[0].clone());
            output.push(SavedItem::InboundErrorSync {
                target: id.clone(),
                error: SavedSyncError::UnitErr {
                    message: text.trim().to_string(),
                },
            });
            output
        }
        _ => items.to_vec(),
    };
    Ok(output)
}

fn output_store_read_error_without_context(id: &str, text: String) -> Vec<SavedItem> {
    vec![SavedItem::InboundErrorSync {
        target: id.to_string(),
        error: SavedSyncError::StoreReadErr {
            message: text.trim().to_string(),
        },
    }]
}
