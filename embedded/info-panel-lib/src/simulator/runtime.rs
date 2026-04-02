use super::codec::*;
use super::forms::*;
use super::replay::*;
use super::types::{
    AsyncOp, AsyncResult, InboundAsyncKind, InfoPanelBundle, InfoPanelSpec, SyncError, SyncOp,
    SyncResult,
};
use super::*;
use simulator::editor::{replay_steps_for_trace, EditorChoice, FormSpec, FormState, RuntimeTarget};

pub struct InfoPanelSimulatorRuntime;

impl InfoPanelSimulatorRuntime {
    pub fn new() -> Self {
        Self
    }
}

type RunItem = simulator::editor::TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult>;
type Choice = EditorChoice<SyncOp, AsyncOp, InboundAsyncKind>;

fn runtime_removal_span(document: &[RunItem], item_index: usize) -> Result<(usize, usize), String> {
    let Some(item) = document.get(item_index) else {
        return Err(format!("invalid item index {item_index}"));
    };
    match item {
        simulator::editor::TraceItem::InboundReturnSync { target, .. }
        | simulator::editor::TraceItem::InboundErrorSync { target, .. }
        | simulator::editor::TraceItem::InboundResolveAsync { target, .. }
        | simulator::editor::TraceItem::InboundAbortAsync { target }
        | simulator::editor::TraceItem::InboundCancelAsync { target } => {
            if item_index > 0 {
                match &document[item_index - 1] {
                    simulator::editor::TraceItem::OutboundCreateSync { id, .. }
                    | simulator::editor::TraceItem::OutboundCreateAsync { id, .. }
                        if id == target =>
                    {
                        return Ok((item_index - 1, item_index + 1));
                    }
                    _ => {}
                }
            }
            Ok((item_index, item_index + 1))
        }
        _ => Ok((item_index, item_index + 1)),
    }
}

fn current_item_for_target<'a>(
    document: &'a [RunItem],
    target: &RuntimeTarget,
) -> Option<&'a RunItem> {
    match target {
        RuntimeTarget::Insert { .. } => None,
        RuntimeTarget::Edit { item_index } => document.get(*item_index),
    }
}

fn current_ticks_for_target(
    runtime: &InfoPanelSimulatorRuntime,
    document: &[RunItem],
    target: &RuntimeTarget,
) -> Result<u64, String> {
    let prefix = match target {
        RuntimeTarget::Insert { insertion_index } => &document[..*insertion_index],
        RuntimeTarget::Edit { item_index } => {
            let (start, _) = runtime_removal_span(document, *item_index)?;
            &document[..start]
        }
    };
    let replay = replay_steps_for_trace(runtime, prefix)?;
    Ok(current_ticks_from_trace(&replay))
}

impl TraceRuntime for InfoPanelSimulatorRuntime {
    type SyncOp = SyncOp;
    type AsyncOp = AsyncOp;
    type SyncResult = SyncResult;
    type SyncError = SyncError;
    type AsyncResult = AsyncResult;
    type Bundle = InfoPanelBundle;
    type ReplaySpec = InfoPanelSpec;

    fn form_schema(
        &self,
        document: &[RunItem],
        target: &RuntimeTarget,
        choice: &Choice,
    ) -> Result<FormSpec, String> {
        let title = match target {
            RuntimeTarget::Insert { .. } => "Insert event",
            RuntimeTarget::Edit { .. } => "Edit event",
        };
        let current_ticks = current_ticks_for_target(self, document, target)?;
        Ok(form_spec_for_choice(
            title,
            choice,
            current_item_for_target(document, target),
            current_ticks,
        ))
    }

    fn decode_form_state(
        &self,
        document: &[RunItem],
        target: &RuntimeTarget,
        choice: &Choice,
        state: &FormState,
    ) -> Result<Vec<RunItem>, String> {
        let current_ticks = current_ticks_for_target(self, document, target)?;
        runtime_items_from_form_state(choice, state, current_ticks)
    }

    fn format_editor_choice(&self, choice: &Choice) -> String {
        match choice {
            EditorChoice::ReturnSyncSuccess { target, op, .. } => {
                format!("ReturnSync#{target} {}", format_sync_op(op))
            }
            EditorChoice::ReturnSyncError { target, op, .. } => {
                format!("ErrorSync#{target} {}", format_sync_op(op))
            }
            EditorChoice::ResolveAsync {
                target,
                op,
                warnings,
                ..
            } => {
                if warnings.is_empty() {
                    format!("ResolveAsync#{target} {}", format_async_op(op))
                } else {
                    format!(
                        "ResolveAsync#{target} {} warnings={warnings:?}",
                        format_async_op(op)
                    )
                }
            }
            EditorChoice::AbortAsync { target, op, .. } => {
                format!("AbortAsync#{target} {}", format_async_op(op))
            }
            EditorChoice::CreateInboundAsync { kind, .. } => {
                format!("CreateInboundAsync {}", inbound_async_kind_name(kind))
            }
            EditorChoice::CancelInboundAsync { target, op } => {
                format!("CancelInboundAsync#{target} {}", format_async_op(op))
            }
            EditorChoice::DropResult { target, outbound } => {
                if *outbound {
                    format!("DropResult#{target}")
                } else {
                    format!("InboundDropResult#{target}")
                }
            }
        }
    }

    fn default_sync_error(&self, op: &Self::SyncOp) -> Option<Self::SyncError> {
        default_runtime_sync_error(op)
    }

    fn new_replay_bundle(&self) -> Self::Bundle {
        InfoPanelBundle::new()
    }

    fn sync_error_to_result(&self, error: &Self::SyncError) -> Self::SyncResult {
        SavedSyncError::from_runtime_error(error).to_runtime_result()
    }

    fn inbound_async_kind(&self, op: &Self::AsyncOp) -> Option<InboundAsyncKind> {
        match op {
            AsyncOp::PortalHttpRequest { .. } => Some(InboundAsyncKind::PortalHttpRequest),
            AsyncOp::PortalClientConnected => Some(InboundAsyncKind::PortalClientConnected),
            AsyncOp::PortalStopped => Some(InboundAsyncKind::PortalStopped),
            _ => None,
        }
    }

    fn format_trace_item(
        &self,
        item: &simulator::editor::TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult>,
    ) -> String {
        format_saved_item(&saved_item_from_runtime_item(item))
    }

    fn format_runtime_event(
        &self,
        event: &Event<SyncOp, AsyncOp, SyncResult, AsyncResult>,
    ) -> String {
        format_event(event)
    }

    fn sync_op_result_target(&self, op: &Self::SyncOp) -> Option<String> {
        match op {
            SyncOp::HttpRead { body, .. } => Some(body.clone()),
            _ => None,
        }
    }

    fn async_op_result_target(&self, _op: &Self::AsyncOp) -> Option<String> {
        None
    }

    fn async_op_to_json(&self, value: &Self::AsyncOp) -> Result<serde_json::Value, String> {
        serde_json::to_value(SavedAsyncOp::from_runtime(value)).map_err(|err| err.to_string())
    }

    fn async_op_from_json(&self, value: serde_json::Value) -> Result<Self::AsyncOp, String> {
        serde_json::from_value::<SavedAsyncOp>(value)
            .map_err(|err| err.to_string())
            .map(|value| value.to_runtime())
    }

    fn sync_result_to_json(&self, value: &Self::SyncResult) -> Result<serde_json::Value, String> {
        let saved = SavedSyncResult::from_runtime(value)?;
        serde_json::to_value(saved).map_err(|err| err.to_string())
    }

    fn sync_result_from_json(&self, value: serde_json::Value) -> Result<Self::SyncResult, String> {
        serde_json::from_value::<SavedSyncResult>(value)
            .map_err(|err| err.to_string())?
            .to_runtime()
    }

    fn sync_error_to_json(&self, value: &Self::SyncError) -> Result<serde_json::Value, String> {
        serde_json::to_value(SavedSyncError::from_runtime_error(value))
            .map_err(|err| err.to_string())
    }

    fn sync_error_from_json(&self, value: serde_json::Value) -> Result<Self::SyncError, String> {
        serde_json::from_value::<SavedSyncError>(value)
            .map_err(|err| err.to_string())
            .map(|value| value.to_runtime_error())
    }

    fn async_result_to_json(&self, value: &Self::AsyncResult) -> Result<serde_json::Value, String> {
        serde_json::to_value(SavedAsyncResult::from_runtime(value)).map_err(|err| err.to_string())
    }

    fn async_result_from_json(
        &self,
        value: serde_json::Value,
    ) -> Result<Self::AsyncResult, String> {
        serde_json::from_value::<SavedAsyncResult>(value)
            .map_err(|err| err.to_string())
            .map(|value| value.to_runtime())
    }
}

#[cfg(test)]
fn saved_items_from_runtime_trace(trace: &[RunItem]) -> Vec<SavedItem> {
    trace.iter().map(saved_item_from_runtime_item).collect()
}

#[cfg(test)]
fn runtime_items_from_saved_items(items: &[SavedItem]) -> Result<Vec<RunItem>, String> {
    items.iter().map(runtime_item_from_saved_item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::forms::{form_spec_for_choice, runtime_items_from_form_state};
    use crate::simulator::types::{AsyncOp, AsyncResult, SyncOp};
    use simulator::editor::{
        editor_choices_for_target, form_state_from_spec, open_trace, render_trace, update, Command,
        RuntimeTarget, VisibleRow,
    };

    fn saved_items(document: &RunDocument) -> Vec<SavedItem> {
        saved_items_from_runtime_trace(document)
    }

    fn runtime_document(items: &[SavedItem]) -> RunDocument {
        runtime_items_from_saved_items(items).expect("items should convert to runtime trace")
    }

    fn infer_choice_from_runtime_items(items: &[RunItem]) -> Choice {
        match items {
            [simulator::editor::TraceItem::OutboundCreateSync {
                id, op: Some(op), ..
            }, simulator::editor::TraceItem::InboundReturnSync { .. }] => {
                EditorChoice::ReturnSyncSuccess {
                    target: id.clone(),
                    op: op.clone(),
                    include_outbound: true,
                }
            }
            [simulator::editor::TraceItem::InboundReturnSync { target, result }] => {
                EditorChoice::ReturnSyncSuccess {
                    target: target.clone(),
                    op: infer_sync_op_from_result(result),
                    include_outbound: false,
                }
            }
            [simulator::editor::TraceItem::OutboundCreateSync {
                id, op: Some(op), ..
            }, simulator::editor::TraceItem::InboundErrorSync { .. }] => {
                EditorChoice::ReturnSyncError {
                    target: id.clone(),
                    op: op.clone(),
                    include_outbound: true,
                }
            }
            [simulator::editor::TraceItem::InboundErrorSync { target, error }] => {
                EditorChoice::ReturnSyncError {
                    target: target.clone(),
                    op: infer_sync_op_from_error(error),
                    include_outbound: false,
                }
            }
            [simulator::editor::TraceItem::OutboundCreateAsync {
                id, op: Some(op), ..
            }, simulator::editor::TraceItem::InboundResolveAsync { .. }] => {
                EditorChoice::ResolveAsync {
                    target: id.clone(),
                    op: op.clone(),
                    include_outbound: true,
                    warnings: Vec::new(),
                }
            }
            [simulator::editor::TraceItem::InboundResolveAsync { target, result }] => {
                EditorChoice::ResolveAsync {
                    target: target.clone(),
                    op: infer_async_op_from_result(result),
                    include_outbound: false,
                    warnings: Vec::new(),
                }
            }
            [simulator::editor::TraceItem::OutboundCreateAsync {
                id, op: Some(op), ..
            }, simulator::editor::TraceItem::InboundAbortAsync { .. }] => {
                EditorChoice::AbortAsync {
                    target: id.clone(),
                    op: op.clone(),
                    include_outbound: true,
                }
            }
            [simulator::editor::TraceItem::InboundAbortAsync { target }] => {
                EditorChoice::AbortAsync {
                    target: target.clone(),
                    op: AsyncOp::Sleep(EmbassyDuration::from_ticks(0)),
                    include_outbound: false,
                }
            }
            [simulator::editor::TraceItem::InboundCancelAsync { target }] => {
                EditorChoice::CancelInboundAsync {
                    target: target.clone(),
                    op: AsyncOp::PortalStopped,
                }
            }
            [simulator::editor::TraceItem::OutboundDropResult { target }] => {
                EditorChoice::DropResult {
                    target: target.clone(),
                    outbound: true,
                }
            }
            [simulator::editor::TraceItem::InboundDropResult { target }] => {
                EditorChoice::DropResult {
                    target: target.clone(),
                    outbound: false,
                }
            }
            [simulator::editor::TraceItem::InboundCreateAsync { id, op, .. }] => {
                EditorChoice::CreateInboundAsync {
                    id: id.clone(),
                    kind: match op {
                        AsyncOp::PortalHttpRequest { .. } => InboundAsyncKind::PortalHttpRequest,
                        AsyncOp::PortalClientConnected => InboundAsyncKind::PortalClientConnected,
                        AsyncOp::PortalStopped => InboundAsyncKind::PortalStopped,
                        _ => panic!("unsupported inbound async op"),
                    },
                }
            }
            _ => panic!("unsupported test items shape"),
        }
    }

    fn infer_sync_op_from_result(result: &SyncResult) -> SyncOp {
        match result {
            SyncResult::BootReason(_) => SyncOp::BootReason,
            SyncResult::Now(_) => SyncOp::Now,
            SyncResult::StoreRead(_) => SyncOp::StoreRead {
                namespace: String::new(),
                keys: Vec::new(),
            },
            SyncResult::MacAddress(_) => SyncOp::MacAddress,
            SyncResult::HttpRead { .. } => SyncOp::HttpRead {
                body: String::new(),
                max_len: 0,
            },
            SyncResult::Unit(_) => SyncOp::TftSetRstHigh,
        }
    }

    fn infer_sync_op_from_error(error: &SyncError) -> SyncOp {
        match error {
            SyncError::StoreReadErr { .. } => SyncOp::StoreRead {
                namespace: String::new(),
                keys: Vec::new(),
            },
            SyncError::UnitErr { .. } => SyncOp::TftSetRstHigh,
        }
    }

    fn infer_async_op_from_result(result: &AsyncResult) -> AsyncOp {
        match result {
            AsyncResult::SleepDone => AsyncOp::Sleep(EmbassyDuration::from_ticks(0)),
            AsyncResult::Unit => AsyncOp::WifiDisconnect,
            AsyncResult::PortalSignal => AsyncOp::PortalStopped,
            AsyncResult::ScanNetworks(_) => AsyncOp::WifiScanNetworks,
            AsyncResult::ConnectionInfo(_) => AsyncOp::WifiConnect {
                timeout: Duration::from_secs(0),
            },
            AsyncResult::PortalStartAccessPoint(_) => AsyncOp::PortalStartAccessPoint {
                ssid: String::new(),
            },
            AsyncResult::HttpResponse { .. } => AsyncOp::HttpGet { url: String::new() },
            AsyncResult::PortalHttpResponse { .. } => AsyncOp::PortalHttpRequest {
                method: "GET".to_string(),
                path: "/".to_string(),
                body: Vec::new(),
            },
        }
    }

    fn form_spec_for_items(title: &str, items: &[SavedItem]) -> FormSpec {
        let runtime_items = runtime_document(items);
        let choice = infer_choice_from_runtime_items(&runtime_items);
        form_spec_for_choice(title, &choice, runtime_items.last(), 0)
    }

    fn default_form_state_for_items(items: &[SavedItem]) -> FormState {
        form_state_from_spec(&form_spec_for_items("Test", items))
    }

    fn encode_items_from_form_state(
        items: &[SavedItem],
        state: &FormState,
    ) -> Result<Vec<SavedItem>, String> {
        let runtime_items = runtime_document(items);
        let choice = infer_choice_from_runtime_items(&runtime_items);
        let encoded = runtime_items_from_form_state(&choice, state, 0)?;
        Ok(saved_items_from_runtime_trace(&encoded))
    }

    fn insert_choice_items(
        runtime: &InfoPanelSimulatorRuntime,
        document: &RunDocument,
        insertion_index: usize,
        choice_index: usize,
    ) -> RunDocument {
        let target = RuntimeTarget::Insert { insertion_index };
        let choices =
            editor_choices_for_target(runtime, document, &target).expect("choices should resolve");
        let spec = runtime
            .form_schema(document, &target, &choices[choice_index])
            .expect("form schema should load");
        let state = form_state_from_spec(&spec);
        runtime
            .decode_form_state(document, &target, &choices[choice_index], &state)
            .expect("encoding should succeed")
    }

    fn apply_form_items(document: &mut RunDocument, target: &RuntimeTarget, items: RunDocument) {
        let mut saved_document = saved_items(document);
        let items = saved_items(&items);
        match target {
            RuntimeTarget::Insert { insertion_index } => {
                for (offset, item) in items.into_iter().enumerate() {
                    saved_document.insert(insertion_index + offset, item);
                }
            }
            RuntimeTarget::Edit { item_index } => {
                let (start, end) = saved_removal_span(&saved_document, *item_index)
                    .expect("edit span should resolve");
                saved_document.splice(start..end, items);
            }
        }
        *document = runtime_document(&saved_document);
    }

    fn saved_removal_span(
        items: &[SavedItem],
        item_index: usize,
    ) -> Result<(usize, usize), String> {
        let Some(item) = items.get(item_index) else {
            return Err(format!("invalid item index {item_index}"));
        };
        match item {
            SavedItem::InboundReturnSync { target, .. }
            | SavedItem::InboundErrorSync { target, .. }
            | SavedItem::InboundResolveAsync { target, .. }
            | SavedItem::InboundAbortAsync { target }
            | SavedItem::InboundCancelAsync { target } => {
                if item_index > 0 {
                    match &items[item_index - 1] {
                        SavedItem::OutboundCreateSync { id, .. }
                        | SavedItem::OutboundCreateAsync { id, .. }
                            if id == target =>
                        {
                            return Ok((item_index - 1, item_index + 1));
                        }
                        _ => {}
                    }
                }
                Ok((item_index, item_index + 1))
            }
            _ => Ok((item_index, item_index + 1)),
        }
    }

    #[test]
    fn inserting_choice_creates_symbolic_outbound_and_target() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let mut document = Vec::new();

        let _choices = editor_choices_for_target(
            &runtime,
            &document,
            &RuntimeTarget::Insert { insertion_index: 0 },
        )
        .expect("choices should load");
        let choice_index = 0;
        let saved_json_items = insert_choice_items(&runtime, &document, 0, choice_index);
        apply_form_items(
            &mut document,
            &RuntimeTarget::Insert { insertion_index: 0 },
            saved_json_items,
        );

        let items = saved_items(&document);
        assert_eq!(items.len(), 2);
        match (&items[0], &items[1]) {
            (
                SavedItem::OutboundCreateSync { id, .. }
                | SavedItem::OutboundCreateAsync { id, .. },
                SavedItem::InboundReturnSync { target, .. }
                | SavedItem::InboundResolveAsync { target, .. }
                | SavedItem::InboundAbortAsync { target }
                | SavedItem::InboundCancelAsync { target },
            ) => {
                assert_eq!(id, target);
                assert!(id.chars().any(|ch| ch.is_ascii_alphabetic()));
            }
            other => panic!("unexpected saved items: {other:?}"),
        }

        let first = serde_json::to_value(&items[0]).expect("first item should serialize");
        let first = first.as_object().expect("first item should be object");
        assert_eq!(first.get("type").and_then(|v| v.as_str()), Some("outbound"));
        assert!(matches!(
            first.get("event_type").and_then(|v| v.as_str()),
            Some("create_sync") | Some("create_async")
        ));
        let second = serde_json::to_value(&items[1]).expect("second item should serialize");
        let second = second.as_object().expect("second item should be object");
        assert_eq!(second.get("type").and_then(|v| v.as_str()), Some("inbound"));
        assert!(matches!(
            second.get("event_type").and_then(|v| v.as_str()),
            Some("return_sync")
                | Some("resolve_async")
                | Some("abort_async")
                | Some("cancel_async")
        ));
    }

    #[test]
    fn rendered_rows_hide_script_outbound_marker_and_omit_ids() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let mut document = Vec::new();
        let choices = editor_choices_for_target(
            &runtime,
            &document,
            &RuntimeTarget::Insert { insertion_index: 0 },
        )
        .expect("choices should load");
        let choice_index = choices
            .iter()
            .position(|choice| {
                let label = runtime.format_editor_choice(choice);
                label.starts_with("ResolveAsync") || label.starts_with("ReturnSync")
            })
            .expect("expected a completion choice");
        let saved_json_items = insert_choice_items(&runtime, &document, 0, choice_index);
        apply_form_items(
            &mut document,
            &RuntimeTarget::Insert { insertion_index: 0 },
            saved_json_items,
        );

        let rendered = render_trace(&runtime, &document).expect("rows should render");
        assert!(!rendered
            .rows
            .iter()
            .any(|row| row.text.contains("OUTBOUND")));
        assert!(rendered
            .rows
            .iter()
            .any(|row| row.text.starts_with("INBOUND ")));
        assert!(!rendered.rows.iter().any(|row| row.text.contains('#')));
        assert!(!rendered.rows.iter().any(|row| row.text.contains("->")));
    }

    #[test]
    fn boot_reason_form_can_change_saved_value() {
        let items = vec![
            SavedItem::OutboundCreateSync {
                id: "boot_reason".to_string(),
                target: None,
                op: Some(SavedSyncOp::BootReason),
            },
            SavedItem::InboundReturnSync {
                target: "boot_reason".to_string(),
                result: SavedSyncResult::BootReason {
                    value: "software".to_string(),
                },
            },
        ];
        let spec = form_spec_for_items("Edit event", &items);
        let mut state = default_form_state_for_items(&items);
        state.insert(
            "boot_reason".to_string(),
            simulator::editor::FormValue::Select(1),
        );
        assert!(simulator::editor::form_is_complete(&spec, &state));
        assert!(!simulator::editor::form_is_auto_acceptable(&spec, &state));
        let items = encode_items_from_form_state(&items, &state).expect("save should succeed");

        assert!(matches!(
            &items[1],
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::BootReason { value },
                ..
            } if value == "external_pin"
        ));
    }

    #[test]
    fn store_read_uses_store_read_form_not_boot_reason_form() {
        let items = vec![
            SavedItem::OutboundCreateSync {
                id: "store_read".to_string(),
                target: None,
                op: Some(SavedSyncOp::StoreRead {
                    namespace: "app_config".to_string(),
                    keys: vec!["ssid".to_string(), "pw".to_string()],
                }),
            },
            SavedItem::InboundReturnSync {
                target: "store_read".to_string(),
                result: SavedSyncResult::StoreReadOk {
                    values: BTreeMap::from([
                        ("ssid".to_string(), "old".to_string()),
                        ("pw".to_string(), "secret".to_string()),
                    ]),
                },
            },
        ];
        let mut state = default_form_state_for_items(&items);
        state.insert(
            "text".to_string(),
            simulator::editor::FormValue::Text("ssid=new\npw=updated".to_string()),
        );
        let items = encode_items_from_form_state(&items, &state).expect("save should succeed");

        assert!(matches!(
            &items[1],
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::StoreReadOk { values },
                ..
            } if values.get("ssid") == Some(&"new".to_string()) && values.get("pw") == Some(&"updated".to_string())
        ));
    }

    #[test]
    fn store_read_form_can_return_error() {
        let items = vec![
            SavedItem::OutboundCreateSync {
                id: "store_read".to_string(),
                target: None,
                op: Some(SavedSyncOp::StoreRead {
                    namespace: "app_config".to_string(),
                    keys: vec!["ssid".to_string()],
                }),
            },
            SavedItem::InboundReturnSync {
                target: "store_read".to_string(),
                result: SavedSyncResult::StoreReadOk {
                    values: BTreeMap::from([("ssid".to_string(), "old".to_string())]),
                },
            },
        ];
        let error_items = vec![
            items[0].clone(),
            SavedItem::InboundErrorSync {
                target: "store_read".to_string(),
                error: SavedSyncError::StoreReadErr {
                    message: "old error".to_string(),
                },
            },
        ];
        let mut state = default_form_state_for_items(&error_items);
        state.insert(
            "text".to_string(),
            simulator::editor::FormValue::Text("nvs failed".to_string()),
        );
        let items =
            encode_items_from_form_state(&error_items, &state).expect("save should succeed");

        assert!(matches!(
            &items[1],
            SavedItem::InboundErrorSync {
                error: SavedSyncError::StoreReadErr { message },
                ..
            } if message == "nvs failed"
        ));
    }

    #[test]
    fn tft_sync_form_can_return_error() {
        let items = vec![
            SavedItem::OutboundCreateSync {
                id: "tft_set_rst_high".to_string(),
                target: None,
                op: Some(SavedSyncOp::TftSetRstHigh),
            },
            SavedItem::InboundReturnSync {
                target: "tft_set_rst_high".to_string(),
                result: SavedSyncResult::UnitOk,
            },
        ];
        let error_items = vec![
            items[0].clone(),
            SavedItem::InboundErrorSync {
                target: "tft_set_rst_high".to_string(),
                error: SavedSyncError::UnitErr {
                    message: "old error".to_string(),
                },
            },
        ];
        let mut state = default_form_state_for_items(&error_items);
        state.insert(
            "text".to_string(),
            simulator::editor::FormValue::Text("spi timeout".to_string()),
        );
        let items =
            encode_items_from_form_state(&error_items, &state).expect("save should succeed");

        assert!(matches!(
            &items[1],
            SavedItem::InboundErrorSync {
                error: SavedSyncError::UnitErr { message },
                ..
            } if message == "spi timeout"
        ));
    }

    #[test]
    fn tft_write_serializes_as_hex_string() {
        let value = serde_json::to_value(SavedSyncOp::TftWrite {
            bytes: vec![0x00, 0x0f, 0xa5, 0xff],
        })
        .expect("serialize should succeed");

        assert_eq!(
            value.get("type").and_then(|v| v.as_str()),
            Some("tft_write")
        );
        assert_eq!(
            value.get("bytes_hex").and_then(|v| v.as_str()),
            Some("000fa5ff")
        );
        assert!(value.get("bytes").is_none());

        let roundtrip: SavedSyncOp =
            serde_json::from_value(value).expect("deserialize should succeed");
        assert_eq!(
            roundtrip,
            SavedSyncOp::TftWrite {
                bytes: vec![0x00, 0x0f, 0xa5, 0xff]
            }
        );
    }

    #[test]
    fn http_read_result_serializes_as_hex_string() {
        let value = serde_json::to_value(SavedSyncResult::HttpRead {
            bytes: vec![0x00, 0x0f, 0xa5, 0xff],
        })
        .expect("serialize should succeed");

        assert_eq!(
            value.get("type").and_then(|v| v.as_str()),
            Some("http_read")
        );
        assert_eq!(
            value.get("bytes_hex").and_then(|v| v.as_str()),
            Some("000fa5ff")
        );
        assert!(value.get("bytes").is_none());

        let roundtrip: SavedSyncResult =
            serde_json::from_value(value).expect("deserialize should succeed");
        assert_eq!(
            roundtrip,
            SavedSyncResult::HttpRead {
                bytes: vec![0x00, 0x0f, 0xa5, 0xff]
            }
        );
    }

    #[test]
    fn async_saved_values_roundtrip_full_runtime_information() {
        let networks = SavedAsyncResult::from_runtime(&AsyncResult::ScanNetworks(vec![
            wifi::FoundNetwork::new("ssid-a", Some(1), Some(-30)),
            wifi::FoundNetwork::new("ssid-b", None, None),
        ]));
        assert_eq!(
            networks,
            SavedAsyncResult::ScanNetworks {
                networks: vec![
                    SavedFoundNetwork {
                        ssid: "ssid-a".to_string(),
                        channel: Some(1),
                        signal_strength: Some(-30),
                    },
                    SavedFoundNetwork {
                        ssid: "ssid-b".to_string(),
                        channel: None,
                        signal_strength: None,
                    },
                ],
            }
        );
        assert_eq!(
            networks.to_runtime(),
            AsyncResult::ScanNetworks(vec![
                wifi::FoundNetwork::new("ssid-a", Some(1), Some(-30)),
                wifi::FoundNetwork::new("ssid-b", None, None),
            ])
        );

        let response = SavedAsyncResult::from_runtime(&AsyncResult::PortalHttpResponse {
            status_code: 201,
            content_type: "application/json",
            body_len: 42,
        });
        assert_eq!(
            response,
            SavedAsyncResult::PortalHttpResponse {
                status_code: 201,
                content_type: "application/json".to_string(),
                body_len: 42,
            }
        );
    }

    #[test]
    fn async_saved_ops_roundtrip_full_runtime_information() {
        let sleep = SavedAsyncOp::from_runtime(&AsyncOp::Sleep(EmbassyDuration::from_ticks(1234)));
        assert_eq!(
            sleep,
            SavedAsyncOp::Sleep {
                duration: SavedEmbassyDuration { ticks: 1234 },
            }
        );
        assert_eq!(
            sleep.to_runtime(),
            AsyncOp::Sleep(EmbassyDuration::from_ticks(1234))
        );

        let connect = SavedAsyncOp::from_runtime(&AsyncOp::WifiConnect {
            timeout: Duration::new(2, 345),
        });
        assert_eq!(
            connect,
            SavedAsyncOp::WifiConnect {
                timeout: SavedStdDuration {
                    secs: 2,
                    nanos: 345
                },
            }
        );
        assert_eq!(
            connect.to_runtime(),
            AsyncOp::WifiConnect {
                timeout: Duration::new(2, 345),
            }
        );
    }

    #[test]
    fn now_default_uses_elapsed_time_from_trace_prefix() {
        let current_ticks = current_ticks_from_trace(&[
            TraceStep::start(vec![Event::CreateAsync {
                id: 1,
                op: AsyncOp::Sleep(EmbassyDuration::from_millis(150)),
            }]),
            TraceStep::push(
                Event::ResolveAsync {
                    id: 1,
                    result: AsyncResult::SleepDone,
                },
                vec![],
            ),
        ]);

        assert_eq!(
            default_runtime_sync_result(&SyncOp::Now, current_ticks),
            SyncResult::Now(EmbassyDuration::from_millis(150).as_ticks())
        );

        let items = vec![
            SavedItem::OutboundCreateSync {
                id: "now".to_string(),
                target: None,
                op: Some(SavedSyncOp::Now),
            },
            SavedItem::InboundReturnSync {
                target: "now".to_string(),
                result: SavedSyncResult::Now {
                    ticks: EmbassyDuration::from_millis(150).as_ticks(),
                },
            },
        ];
        let spec = form_spec_for_items("Insert event", &items);
        let state = default_form_state_for_items(&items);
        assert!(simulator::editor::form_is_auto_acceptable(&spec, &state));
    }

    #[test]
    fn boot_reason_defaults_are_prefilled_but_not_trivial() {
        let items = vec![
            SavedItem::OutboundCreateSync {
                id: "boot_reason".to_string(),
                target: None,
                op: Some(SavedSyncOp::BootReason),
            },
            SavedItem::InboundReturnSync {
                target: "boot_reason".to_string(),
                result: SavedSyncResult::BootReason {
                    value: "software".to_string(),
                },
            },
        ];
        let spec = form_spec_for_items("Insert event", &items);
        let state = default_form_state_for_items(&items);
        assert!(simulator::editor::form_is_complete(&spec, &state));
        assert!(!simulator::editor::form_is_auto_acceptable(&spec, &state));
    }

    #[test]
    fn invalid_trace_still_renders_and_marks_invalid_rows() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let document = vec![SavedItem::OutboundCreateAsync {
            id: "wifi_disconnect_2".to_string(),
            target: None,
            op: None,
        }];
        let document = runtime_document(&document);

        let rendered = render_trace(&runtime, &document).expect("render should succeed");
        assert!(rendered.replay_error.is_some());
        assert!(rendered.rows.iter().any(|row| row.is_invalid));
    }

    #[test]
    fn invalid_inbound_item_still_renders_as_invalid_suffix() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let document = vec![
            SavedItem::OutboundCreateSync {
                id: "now".to_string(),
                target: None,
                op: Some(SavedSyncOp::Now),
            },
            SavedItem::InboundReturnSync {
                target: "now".to_string(),
                result: SavedSyncResult::Now { ticks: 0 },
            },
            SavedItem::InboundCreateAsync {
                id: "portal_client_connected".to_string(),
                target: None,
                op: SavedAsyncOp::PortalClientConnected,
            },
        ];
        let document = runtime_document(&document);

        let rendered = render_trace(&runtime, &document).expect("render should succeed");
        assert!(rendered.replay_error.is_some());
        assert!(rendered
            .rows
            .iter()
            .any(|row| row.is_invalid && row.text.contains("PortalClientConnected")));
        let invalid_indices = rendered
            .rows
            .iter()
            .filter(|row| row.is_invalid)
            .map(|row| row.insertion_index)
            .collect::<Vec<_>>();
        assert_eq!(invalid_indices.last().copied(), Some(3));
    }

    #[test]
    fn trace_after_boot_reason_includes_tft_events() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let document = vec![
            SavedItem::OutboundCreateSync {
                id: "boot_reason".to_string(),
                target: None,
                op: Some(SavedSyncOp::BootReason),
            },
            SavedItem::InboundReturnSync {
                target: "boot_reason".to_string(),
                result: SavedSyncResult::BootReason {
                    value: "software".to_string(),
                },
            },
        ];
        let document = runtime_document(&document);

        let rendered = render_trace(&runtime, &document).expect("render should succeed");
        assert!(rendered.rows.iter().any(|row| {
            row.text.contains("TftSetRstHigh")
                || row.text.contains("TftSetRstLow")
                || row.text.contains("TftSetDcLow")
                || row.text.contains("TftSetDcHigh")
                || row.text.contains("TftWrite(len=")
        }));
    }

    #[test]
    fn defaulted_choices_can_be_materialized_directly() {
        let items = vec![
            SavedItem::OutboundCreateSync {
                id: "boot_reason".to_string(),
                target: None,
                op: Some(SavedSyncOp::BootReason),
            },
            SavedItem::InboundReturnSync {
                target: "boot_reason".to_string(),
                result: SavedSyncResult::BootReason {
                    value: "software".to_string(),
                },
            },
        ];
        let spec = form_spec_for_items("Insert event", &items);
        let state = default_form_state_for_items(&items);
        assert!(simulator::editor::form_is_complete(&spec, &state));
        assert!(!simulator::editor::form_is_auto_acceptable(&spec, &state));

        let encoded =
            encode_items_from_form_state(&items, &state).expect("encoding should succeed");
        assert!(!encoded.is_empty());
    }

    #[test]
    #[ignore = "literal end-of-document choices are not the same as visible tail preview"]
    fn simulator1_tail_has_a_single_defaulted_trivial_choice() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let envelope: simulator::editor::RunEnvelope<SavedItem> =
            serde_json::from_str(include_str!("../../simulator1.json"))
                .expect("simulator1 should parse");
        let document: RunDocument = runtime_document(&envelope.items);
        let insertion_index = document.len();

        let target = RuntimeTarget::Insert { insertion_index };
        let choices =
            editor_choices_for_target(&runtime, &document, &target).expect("choices should load");
        assert!(
            !choices.is_empty(),
            "expected choices at end of simulator1 trace"
        );

        let complete = choices
            .iter()
            .filter_map(|choice| {
                let spec = runtime
                    .form_schema(&document, &target, choice)
                    .expect("form schema should load");
                let state = form_state_from_spec(&spec);
                simulator::editor::form_is_auto_acceptable(&spec, &state)
                    .then_some(runtime.format_editor_choice(choice))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            complete.len(),
            1,
            "expected exactly one default-complete choice at tail, got {complete:?}"
        );
    }

    #[test]
    fn async_resolve_is_trivial_but_abort_is_not() {
        let resolve_items = vec![SavedItem::InboundResolveAsync {
            target: "sleep_150ms".to_string(),
            result: SavedAsyncResult::SleepDone,
        }];
        let resolve_spec = form_spec_for_items("Insert event", &resolve_items);
        let resolve_state = default_form_state_for_items(&resolve_items);
        assert!(simulator::editor::form_is_auto_acceptable(
            &resolve_spec,
            &resolve_state
        ));

        let abort_items = vec![SavedItem::InboundAbortAsync {
            target: "sleep_150ms".to_string(),
        }];
        let abort_spec = form_spec_for_items("Insert event", &abort_items);
        let abort_state = default_form_state_for_items(&abort_items);
        assert!(!simulator::editor::form_is_auto_acceptable(
            &abort_spec,
            &abort_state
        ));
    }

    #[test]
    #[ignore = "debug snapshot helper"]
    fn debug_render_current_simulator1_states() {
        fn step_count(rows: &[VisibleRow]) -> usize {
            let mut count = 0;
            let mut previous = None;
            for row in rows {
                if previous != Some(row.insertion_index) {
                    count += 1;
                    previous = Some(row.insertion_index);
                }
            }
            count
        }

        let runtime = InfoPanelSimulatorRuntime::new();
        let directory = std::env::temp_dir().join("simulator1-snapshot-debug");
        std::fs::create_dir_all(&directory).expect("debug dir should exist");
        let path = directory.join("simulator1.json");
        std::fs::write(&path, include_str!("../../simulator1.json")).expect("write simulator1");

        let session = open_trace(&runtime, &path, 120, 30).expect("open trace");
        let rendered = render_trace(&runtime, &session.state.view.trace).expect("rendered rows");
        let step_count = step_count(&rendered.rows);
        let previous_step = step_count - 2;
        let last_step = step_count - 1;

        let mut previous_session = open_trace(&runtime, &path, 120, 30).expect("reopen trace");
        previous_session.state.view.cursor_step_index = previous_step;
        let (state, _) = update(previous_session.state, Command::MoveUp, &runtime);
        previous_session.state = state;
        let (state, _) = update(previous_session.state, Command::MoveDown, &runtime);
        previous_session.state = state;
        let (state, _) = update(previous_session.state, Command::StartInsert, &runtime);
        previous_session.state = state;
        println!(
            "--- previous step append ---\n{}",
            simulator::ui::render_state_to_text(&mut previous_session, &runtime, 120, 30)
                .expect("render")
        );

        let mut last_session = session;
        last_session.state.view.cursor_step_index = last_step;
        let (state, _) = update(last_session.state, Command::MoveUp, &runtime);
        last_session.state = state;
        let (state, _) = update(last_session.state, Command::MoveDown, &runtime);
        last_session.state = state;
        println!(
            "--- last step ---\n{}",
            simulator::ui::render_state_to_text(&mut last_session, &runtime, 120, 30)
                .expect("render")
        );

        last_session.state.view.scroll_offset = rendered.rows.len().saturating_sub(8);
        println!(
            "--- last step scrolled ---\n{}",
            simulator::ui::render_state_to_text(&mut last_session, &runtime, 120, 30)
                .expect("render")
        );
    }
}
