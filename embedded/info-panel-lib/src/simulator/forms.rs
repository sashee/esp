use super::replay::*;
use super::types::{AsyncOp, AsyncResult, InboundAsyncKind, SyncError, SyncOp, SyncResult};
use super::*;
use crate::simulator::codec::{format_boot_reason, parse_boot_reason};
use simulator::editor::{
    EditorChoice, FormField, FormFieldKind, FormSpec, FormState, FormValue, TraceItem,
};

const FIELD_BOOT_REASON: &str = "boot_reason";
const FIELD_TICKS: &str = "ticks";
const FIELD_TEXT: &str = "text";
const FIELD_CONFIRM: &str = "confirm";

type RunItem = TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult>;
type Choice = EditorChoice<SyncOp, AsyncOp, InboundAsyncKind>;

pub(super) fn form_spec_for_choice(
    title: &str,
    choice: &Choice,
    current_item: Option<&RunItem>,
    current_ticks: u64,
) -> FormSpec {
    match choice {
        Choice::ReturnSyncSuccess {
            op: SyncOp::BootReason,
            ..
        } => FormSpec {
            title: title.to_string(),
            details: Vec::new(),
            fields: vec![FormField {
                id: FIELD_BOOT_REASON.to_string(),
                label: "Boot reason".to_string(),
                kind: FormFieldKind::Select {
                    options: BOOT_REASON_OPTIONS
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                },
                help: None,
                initial_value: Some(FormValue::Select(current_boot_reason_selection(
                    current_item,
                ))),
            }],
            auto_accept_if_complete: false,
        },
        Choice::ReturnSyncSuccess {
            op: SyncOp::Now, ..
        } => FormSpec {
            title: title.to_string(),
            details: vec!["Ticks since boot.".to_string()],
            fields: vec![FormField {
                id: FIELD_TICKS.to_string(),
                label: "Ticks".to_string(),
                kind: FormFieldKind::Text { multiline: false },
                help: None,
                initial_value: Some(FormValue::Text(
                    current_now_ticks(current_item)
                        .unwrap_or(current_ticks)
                        .to_string(),
                )),
            }],
            auto_accept_if_complete: true,
        },
        Choice::ReturnSyncSuccess {
            op: SyncOp::StoreRead { namespace, keys },
            ..
        } => FormSpec {
            title: title.to_string(),
            details: vec![
                format!("namespace: {namespace}"),
                format!("keys: {}", keys.join(", ")),
            ],
            fields: vec![FormField {
                id: FIELD_TEXT.to_string(),
                label: "Payload".to_string(),
                kind: FormFieldKind::Text { multiline: true },
                help: Some("Success values as key=value lines.".to_string()),
                initial_value: Some(FormValue::Text(current_store_read_success_text(
                    current_item,
                    keys,
                ))),
            }],
            auto_accept_if_complete: false,
        },
        Choice::ReturnSyncError {
            op: SyncOp::StoreRead { namespace, keys },
            ..
        } => FormSpec {
            title: title.to_string(),
            details: vec![
                format!("namespace: {namespace}"),
                format!("keys: {}", keys.join(", ")),
            ],
            fields: vec![FormField {
                id: FIELD_TEXT.to_string(),
                label: "Message".to_string(),
                kind: FormFieldKind::Text { multiline: true },
                help: Some("Error message.".to_string()),
                initial_value: Some(FormValue::Text(current_store_read_error_text(current_item))),
            }],
            auto_accept_if_complete: false,
        },
        Choice::ReturnSyncSuccess { op, .. } if is_unit_sync_op(op) => FormSpec {
            title: title.to_string(),
            details: vec![format!("operation: {}", format_sync_op(op))],
            fields: Vec::new(),
            auto_accept_if_complete: true,
        },
        Choice::ReturnSyncError { op, .. } if is_unit_sync_op(op) => FormSpec {
            title: title.to_string(),
            details: vec![format!("operation: {}", format_sync_op(op))],
            fields: vec![FormField {
                id: FIELD_TEXT.to_string(),
                label: "Message".to_string(),
                kind: FormFieldKind::Text { multiline: true },
                help: Some("Error message.".to_string()),
                initial_value: Some(FormValue::Text(current_unit_error_text(current_item))),
            }],
            auto_accept_if_complete: false,
        },
        Choice::AbortAsync { .. }
        | Choice::CancelInboundAsync { .. }
        | Choice::DropResult { .. } => FormSpec {
            title: title.to_string(),
            details: vec![format_choice_details(choice, current_ticks)],
            fields: vec![FormField {
                id: FIELD_CONFIRM.to_string(),
                label: "Confirm".to_string(),
                kind: FormFieldKind::Toggle {
                    false_label: "no".to_string(),
                    true_label: "yes".to_string(),
                },
                help: Some("Toggle to confirm this non-trivial action.".to_string()),
                initial_value: None,
            }],
            auto_accept_if_complete: false,
        },
        _ => FormSpec {
            title: title.to_string(),
            details: vec![format_choice_details(choice, current_ticks)],
            fields: Vec::new(),
            auto_accept_if_complete: true,
        },
    }
}

pub(super) fn runtime_items_from_form_state(
    choice: &Choice,
    state: &FormState,
    current_ticks: u64,
) -> Result<Vec<RunItem>, String> {
    match choice {
        Choice::ReturnSyncSuccess {
            target,
            op,
            include_outbound,
        } => match op {
            SyncOp::BootReason => Ok(with_optional_outbound_sync(
                target,
                op,
                *include_outbound,
                SyncResult::BootReason(parse_boot_reason_selection(state)),
            )),
            SyncOp::Now => Ok(with_optional_outbound_sync(
                target,
                op,
                *include_outbound,
                SyncResult::Now(parse_ticks(state)?),
            )),
            SyncOp::StoreRead { keys, .. } => Ok(with_optional_outbound_sync(
                target,
                op,
                *include_outbound,
                SyncResult::StoreRead(Ok(parse_store_read_text(keys, &text_value(state))?)),
            )),
            _ => Ok(with_optional_outbound_sync(
                target,
                op,
                *include_outbound,
                default_runtime_sync_result(op, current_ticks),
            )),
        },
        Choice::ReturnSyncError {
            target,
            op,
            include_outbound,
        } => Ok(with_optional_outbound_error_sync(
            target,
            op,
            *include_outbound,
            sync_error_for_choice(op, state)?,
        )),
        Choice::ResolveAsync {
            target,
            op,
            include_outbound,
            ..
        } => Ok(with_optional_outbound_async(
            target,
            op,
            *include_outbound,
            default_runtime_async_result(op, target),
        )),
        Choice::AbortAsync {
            target,
            op,
            include_outbound,
        } => Ok(with_optional_abort_async(target, op, *include_outbound)),
        Choice::CreateInboundAsync { id, kind } => Ok(vec![TraceItem::InboundCreateAsync {
            id: id.clone(),
            target: None,
            op: default_runtime_inbound_async_op(kind),
        }]),
        Choice::CancelInboundAsync { target, .. } => Ok(vec![TraceItem::InboundCancelAsync {
            target: target.clone(),
        }]),
        Choice::DropResult { target, outbound } => Ok(vec![if *outbound {
            TraceItem::OutboundDropResult {
                target: target.clone(),
            }
        } else {
            TraceItem::InboundDropResult {
                target: target.clone(),
            }
        }]),
    }
}

fn with_optional_outbound_sync(
    target: &str,
    op: &SyncOp,
    include_outbound: bool,
    result: SyncResult,
) -> Vec<RunItem> {
    let mut items = Vec::new();
    if include_outbound {
        items.push(TraceItem::OutboundCreateSync {
            id: target.to_string(),
            target: sync_op_target(op),
            op: Some(op.clone()),
        });
    }
    items.push(TraceItem::InboundReturnSync {
        target: target.to_string(),
        result,
    });
    items
}

fn with_optional_outbound_error_sync(
    target: &str,
    op: &SyncOp,
    include_outbound: bool,
    error: SyncError,
) -> Vec<RunItem> {
    let mut items = Vec::new();
    if include_outbound {
        items.push(TraceItem::OutboundCreateSync {
            id: target.to_string(),
            target: sync_op_target(op),
            op: Some(op.clone()),
        });
    }
    items.push(TraceItem::InboundErrorSync {
        target: target.to_string(),
        error,
    });
    items
}

fn with_optional_outbound_async(
    target: &str,
    op: &AsyncOp,
    include_outbound: bool,
    result: AsyncResult,
) -> Vec<RunItem> {
    let mut items = Vec::new();
    if include_outbound {
        items.push(TraceItem::OutboundCreateAsync {
            id: target.to_string(),
            target: async_op_target(op),
            op: Some(op.clone()),
        });
    }
    items.push(TraceItem::InboundResolveAsync {
        target: target.to_string(),
        result,
    });
    items
}

fn with_optional_abort_async(target: &str, op: &AsyncOp, include_outbound: bool) -> Vec<RunItem> {
    let mut items = Vec::new();
    if include_outbound {
        items.push(TraceItem::OutboundCreateAsync {
            id: target.to_string(),
            target: async_op_target(op),
            op: Some(op.clone()),
        });
    }
    items.push(TraceItem::InboundAbortAsync {
        target: target.to_string(),
    });
    items
}

fn current_boot_reason_selection(current_item: Option<&RunItem>) -> usize {
    match current_item {
        Some(TraceItem::InboundReturnSync {
            result: SyncResult::BootReason(value),
            ..
        }) => BOOT_REASON_OPTIONS
            .iter()
            .position(|option| *option == format_boot_reason(*value))
            .unwrap_or(0),
        _ => 0,
    }
}

fn current_now_ticks(current_item: Option<&RunItem>) -> Option<u64> {
    match current_item {
        Some(TraceItem::InboundReturnSync {
            result: SyncResult::Now(ticks),
            ..
        }) => Some(*ticks),
        _ => None,
    }
}

fn current_store_read_success_text(current_item: Option<&RunItem>, keys: &[String]) -> String {
    match current_item {
        Some(TraceItem::InboundReturnSync {
            result: SyncResult::StoreRead(Ok(values)),
            ..
        }) => format_store_read_text(values),
        _ => format_store_read_text(&default_store_values(keys)),
    }
}

fn current_store_read_error_text(current_item: Option<&RunItem>) -> String {
    match current_item {
        Some(TraceItem::InboundErrorSync {
            error: SyncError::StoreReadErr { message },
            ..
        }) => message.clone(),
        _ => String::new(),
    }
}

fn current_unit_error_text(current_item: Option<&RunItem>) -> String {
    match current_item {
        Some(TraceItem::InboundErrorSync {
            error: SyncError::UnitErr { message },
            ..
        }) => message.clone(),
        _ => String::new(),
    }
}

pub(super) fn format_store_read_text(
    values: &std::collections::BTreeMap<String, String>,
) -> String {
    values
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn parse_store_read_text(
    allowed_keys: &[String],
    text: &str,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut values = std::collections::BTreeMap::new();
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

fn parse_boot_reason_selection(state: &FormState) -> crate::BootReason {
    let selected = match state.get(FIELD_BOOT_REASON) {
        Some(FormValue::Select(selected)) => *selected,
        _ => 0,
    };
    parse_boot_reason(
        BOOT_REASON_OPTIONS[selected.min(BOOT_REASON_OPTIONS.len().saturating_sub(1))],
    )
    .expect("known boot reason option should parse")
}

fn parse_ticks(state: &FormState) -> Result<u64, String> {
    match state.get(FIELD_TICKS) {
        Some(FormValue::Text(text)) => text.trim().parse::<u64>().map_err(|err| err.to_string()),
        _ => Err("missing ticks".to_string()),
    }
}

fn text_value(state: &FormState) -> String {
    match state.get(FIELD_TEXT) {
        Some(FormValue::Text(text)) => text.clone(),
        _ => String::new(),
    }
}

fn sync_error_for_choice(op: &SyncOp, state: &FormState) -> Result<SyncError, String> {
    let text = text_value(state).trim().to_string();
    match op {
        SyncOp::StoreRead { .. } => Ok(SyncError::StoreReadErr { message: text }),
        op if is_unit_sync_op(op) => Ok(SyncError::UnitErr { message: text }),
        _ => Err(format!(
            "sync op has no error variant: {}",
            format_sync_op(op)
        )),
    }
}

fn is_unit_sync_op(op: &SyncOp) -> bool {
    matches!(
        op,
        SyncOp::StoreWrite { .. }
            | SyncOp::StoreRemove { .. }
            | SyncOp::TftSetDcLow
            | SyncOp::TftSetDcHigh
            | SyncOp::TftSetRstLow
            | SyncOp::TftSetRstHigh
            | SyncOp::TftWrite { .. }
    )
}

fn sync_op_target(op: &SyncOp) -> Option<String> {
    match op {
        SyncOp::HttpRead { body, .. } => Some(body.clone()),
        _ => None,
    }
}

fn async_op_target(_op: &AsyncOp) -> Option<String> {
    None
}

fn format_choice_details(choice: &Choice, current_ticks: u64) -> String {
    match choice {
        Choice::ReturnSyncSuccess { target: _, op, .. } => format!(
            "INBOUND ReturnSync {:?}",
            default_runtime_sync_result(op, current_ticks)
        ),
        Choice::ReturnSyncError { target: _, op, .. } => match default_runtime_sync_error(op) {
            Some(error) => format!("INBOUND ErrorSync {error:?}"),
            None => format!("INBOUND {}", format_sync_op(op)),
        },
        Choice::ResolveAsync { target, op, .. } => {
            format!(
                "INBOUND ResolveAsync {:?}",
                default_runtime_async_result(op, target)
            )
        }
        Choice::AbortAsync { .. } => "INBOUND AbortAsync".to_string(),
        Choice::CreateInboundAsync { kind, .. } => {
            format!("INBOUND CreateAsync {}", inbound_async_kind_name(kind))
        }
        Choice::CancelInboundAsync { .. } => "INBOUND CancelAsync".to_string(),
        Choice::DropResult { target, outbound } => {
            if *outbound {
                format!("OUTBOUND DropResult {target}")
            } else {
                format!("INBOUND DropResult {target}")
            }
        }
    }
}
