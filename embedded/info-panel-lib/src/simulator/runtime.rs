use super::forms::*;
use super::replay::*;
use super::saved::*;
use super::*;
use simulator::editor::{FormSpec, FormState, RenderedTrace};

pub struct InfoPanelSimulatorRuntime;

#[derive(Clone)]
struct ChoiceTemplate {
    label: String,
    items: Vec<SavedItem>,
}

impl InfoPanelSimulatorRuntime {
    pub fn new() -> Self {
        Self
    }
}

fn choice_templates_for_snapshot(snapshot: &ReplaySnapshot) -> Result<Vec<ChoiceTemplate>, String> {
    let mut templates = Vec::new();
    for possible in &snapshot.possible {
        match possible {
            PossibleEvent::ReturnSync { id, op } => {
                let target = snapshot
                    .runtime_to_symbolic
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| uniquify_id(&snapshot.used_ids, sync_op_name(op)));
                let prefix = if snapshot.runtime_to_symbolic.contains_key(id) {
                    Vec::new()
                } else {
                    vec![SavedItem::OutboundCreateSync {
                        id: target.clone(),
                        op: runtime_sync_op_to_saved(op),
                    }]
                };
                let mut success_items = prefix.clone();
                success_items.push(SavedItem::InboundReturnSync {
                    target: target.clone(),
                    result: default_sync_result(op, snapshot.current_ticks),
                });
                templates.push(ChoiceTemplate {
                    label: format!("ReturnSync#{id} {}", format_sync_op(op)),
                    items: success_items,
                });
                if let Some(error) = default_sync_error(op) {
                    let mut error_items = prefix;
                    error_items.push(SavedItem::InboundErrorSync { target, error });
                    templates.push(ChoiceTemplate {
                        label: format!("ErrorSync#{id} {}", format_sync_op(op)),
                        items: error_items,
                    });
                }
            }
            _ => templates.push(ChoiceTemplate {
                label: format_possible_event(possible),
                items: choice_to_saved_items(
                    &snapshot.used_ids,
                    &snapshot.runtime_to_symbolic,
                    snapshot.current_ticks,
                    possible,
                )?,
            }),
        }
    }
    Ok(templates)
}

fn choice_templates_for_target(
    document: &[SavedItem],
    target: &FormTarget,
) -> Result<Vec<ChoiceTemplate>, String> {
    let items = parse_items(document)?;
    match target {
        FormTarget::Insert { insertion_index } => {
            if *insertion_index > items.len() {
                return Err(format!("invalid insertion index {insertion_index}"));
            }
            let snapshot = replay_items(&items[..*insertion_index])?;
            choice_templates_for_snapshot(&snapshot)
        }
        FormTarget::Edit { item_index } => {
            let (start, end) = removal_span(&items, *item_index)?;
            let mut reduced = items.clone();
            reduced.drain(start..end);
            let snapshot = replay_items(&reduced[..start])?;
            choice_templates_for_snapshot(&snapshot)
        }
    }
}

fn form_items_for_target(
    document: &[SavedItem],
    target: &FormTarget,
    choice_index: usize,
) -> Result<Vec<SavedItem>, String> {
    choice_templates_for_target(document, target)?
        .get(choice_index)
        .map(|choice| choice.items.clone())
        .ok_or_else(|| format!("invalid choice index {choice_index}"))
}

impl TraceRuntime for InfoPanelSimulatorRuntime {
    type SyncOp = SavedSyncOp;
    type AsyncOp = SavedAsyncOp;
    type SyncResult = SavedSyncResult;
    type SyncError = SavedSyncError;
    type AsyncResult = SavedAsyncResult;

    fn render_trace(&self, document: &[SavedItem]) -> Result<RenderedTrace, String> {
        let items = parse_items(document)?;
        let snapshot = replay_items(&items)?;
        Ok(RenderedTrace {
            rows: snapshot.rows,
            replay_error: snapshot.replay_error,
        })
    }

    fn insertion_choices(
        &self,
        document: &[SavedItem],
        insertion_index: usize,
    ) -> Result<Vec<InsertionChoice>, String> {
        let items = parse_items(document)?;
        if insertion_index > items.len() {
            return Err(format!("invalid insertion index {insertion_index}"));
        }
        let snapshot = replay_items(&items[..insertion_index])?;
        Ok(choice_templates_for_snapshot(&snapshot)?
            .iter()
            .map(|choice| InsertionChoice {
                label: choice.label.clone(),
            })
            .collect())
    }

    fn edit_choices(
        &self,
        document: &[SavedItem],
        item_index: usize,
    ) -> Result<Vec<InsertionChoice>, String> {
        let items = parse_items(document)?;
        let (start, end) = removal_span(&items, item_index)?;
        let mut reduced = items.clone();
        reduced.drain(start..end);
        let snapshot = replay_items(&reduced[..start])?;
        Ok(choice_templates_for_snapshot(&snapshot)?
            .iter()
            .map(|choice| InsertionChoice {
                label: choice.label.clone(),
            })
            .collect())
    }

    fn form_spec(
        &self,
        document: &[SavedItem],
        target: &FormTarget,
        choice_index: usize,
    ) -> Result<FormSpec, String> {
        let items = form_items_for_target(document, target, choice_index)?;
        Ok(form_spec_for_items(
            match target {
                FormTarget::Insert { .. } => "Insert event",
                FormTarget::Edit { .. } => "Edit event",
            },
            &items,
        ))
    }

    fn initial_form_state(
        &self,
        document: &[SavedItem],
        target: &FormTarget,
        choice_index: usize,
    ) -> Result<FormState, String> {
        let items = form_items_for_target(document, target, choice_index)?;
        Ok(match target {
            FormTarget::Insert { .. } => default_form_state_for_items(&items),
            FormTarget::Edit { item_index } => {
                edited_form_state_for_items(document, *item_index, &items)
            }
        })
    }

    fn encode_form_state(
        &self,
        document: &[SavedItem],
        target: &FormTarget,
        choice_index: usize,
        state: &FormState,
    ) -> Result<Vec<SavedItem>, String> {
        let items = form_items_for_target(document, target, choice_index)?;
        encode_items_from_form_state(&items, state)
    }

    fn apply_form(
        &self,
        document: &mut RunDocument,
        target: &FormTarget,
        items: Vec<SavedItem>,
    ) -> Result<(), String> {
        let mut saved_items = parse_items(document)?;
        match target {
            FormTarget::Insert { insertion_index } => {
                if *insertion_index > saved_items.len() {
                    return Err(format!("invalid insertion index {insertion_index}"));
                }
                for (offset, item) in items.into_iter().enumerate() {
                    saved_items.insert(insertion_index + offset, item);
                }
            }
            FormTarget::Edit { item_index } => {
                let (start, end) = removal_span(&saved_items, *item_index)?;
                saved_items.splice(start..end, items);
            }
        }
        *document = saved_items;
        Ok(())
    }

    fn delete_items(
        &self,
        document: &mut RunDocument,
        item_indices: Vec<usize>,
    ) -> Result<(), String> {
        let mut items = parse_items(document)?;
        let mut spans = item_indices
            .into_iter()
            .map(|index| removal_span(&items, index))
            .collect::<Result<Vec<_>, _>>()?;
        spans.sort_unstable();
        let mut merged = Vec::<(usize, usize)>::new();
        for (start, end) in spans {
            if let Some((_, last_end)) = merged.last_mut() {
                if start <= *last_end {
                    *last_end = (*last_end).max(end);
                    continue;
                }
            }
            merged.push((start, end));
        }
        for (start, end) in merged.into_iter().rev() {
            items.drain(start..end);
        }
        *document = items;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::forms::{
        default_form_state_for_items, encode_items_from_form_state, form_spec_for_items,
    };
    use crate::simulator::types::{AsyncOp, AsyncResult, SyncOp};
    use simulator::editor::{open_trace, update, Command};

    fn saved_items(document: &RunDocument) -> Vec<SavedItem> {
        parse_items(document).expect("document should parse")
    }

    fn insert_choice_items(
        runtime: &InfoPanelSimulatorRuntime,
        document: &RunDocument,
        insertion_index: usize,
        choice_index: usize,
    ) -> Vec<SavedItem> {
        let target = FormTarget::Insert { insertion_index };
        let _ = runtime
            .form_spec(document, &target, choice_index)
            .expect("form spec should load");
        let state = runtime
            .initial_form_state(document, &target, choice_index)
            .expect("initial form state should load");
        runtime
            .encode_form_state(document, &target, choice_index, &state)
            .expect("encoding should succeed")
    }

    #[test]
    fn inserting_choice_creates_symbolic_outbound_and_target() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let mut document = Vec::new();

        let _choices = runtime
            .insertion_choices(&document, 0)
            .expect("choices should load");
        let choice_index = 0;
        let saved_json_items = insert_choice_items(&runtime, &document, 0, choice_index);
        runtime
            .apply_form(
                &mut document,
                &FormTarget::Insert { insertion_index: 0 },
                saved_json_items,
            )
            .expect("apply form should succeed");

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

        let first = serde_json::to_value(&document[0]).expect("first item should serialize");
        let first = first.as_object().expect("first item should be object");
        assert_eq!(first.get("type").and_then(|v| v.as_str()), Some("outbound"));
        assert!(matches!(
            first.get("event_type").and_then(|v| v.as_str()),
            Some("create_sync") | Some("create_async")
        ));
        let second = serde_json::to_value(&document[1]).expect("second item should serialize");
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
        let choices = runtime
            .insertion_choices(&document, 0)
            .expect("choices should load");
        let choice_index = choices
            .iter()
            .position(|choice| {
                choice.label.starts_with("ResolveAsync") || choice.label.starts_with("ReturnSync")
            })
            .expect("expected a completion choice");
        let saved_json_items = insert_choice_items(&runtime, &document, 0, choice_index);
        runtime
            .apply_form(
                &mut document,
                &FormTarget::Insert { insertion_index: 0 },
                saved_json_items,
            )
            .expect("apply form should succeed");

        let rendered = runtime.render_trace(&document).expect("rows should render");
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
                op: SavedSyncOp::BootReason,
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
                op: SavedSyncOp::StoreRead {
                    namespace: "app_config".to_string(),
                    keys: vec!["ssid".to_string(), "pw".to_string()],
                },
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
                op: SavedSyncOp::StoreRead {
                    namespace: "app_config".to_string(),
                    keys: vec!["ssid".to_string()],
                },
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
                op: SavedSyncOp::TftSetRstHigh,
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
            default_sync_result(&SyncOp::Now, current_ticks),
            SavedSyncResult::Now {
                ticks: EmbassyDuration::from_millis(150).as_ticks()
            }
        );

        let items = vec![
            SavedItem::OutboundCreateSync {
                id: "now".to_string(),
                op: SavedSyncOp::Now,
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
                op: SavedSyncOp::BootReason,
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
            op: SavedAsyncOp::WifiDisconnect,
        }];

        let rendered = runtime
            .render_trace(&document)
            .expect("render should succeed");
        assert!(rendered.replay_error.is_some());
        assert!(rendered.rows.iter().any(|row| row.is_invalid));
    }

    #[test]
    fn invalid_inbound_item_still_renders_as_invalid_suffix() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let document = vec![
            SavedItem::OutboundCreateSync {
                id: "now".to_string(),
                op: SavedSyncOp::Now,
            },
            SavedItem::InboundReturnSync {
                target: "now".to_string(),
                result: SavedSyncResult::Now { ticks: 0 },
            },
            SavedItem::InboundCreateAsync {
                id: "portal_client_connected".to_string(),
                op: SavedAsyncOp::PortalClientConnected,
            },
        ];

        let rendered = runtime
            .render_trace(&document)
            .expect("render should succeed");
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
    fn invalid_trace_still_allows_deletion() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let mut document = vec![SavedItem::OutboundCreateAsync {
            id: "wifi_disconnect_2".to_string(),
            op: SavedAsyncOp::WifiDisconnect,
        }];

        runtime
            .delete_items(&mut document, vec![0])
            .expect("delete should succeed");
        assert!(document.is_empty());
    }

    #[test]
    fn trace_after_boot_reason_includes_tft_events() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let document = vec![
            SavedItem::OutboundCreateSync {
                id: "boot_reason".to_string(),
                op: SavedSyncOp::BootReason,
            },
            SavedItem::InboundReturnSync {
                target: "boot_reason".to_string(),
                result: SavedSyncResult::BootReason {
                    value: "software".to_string(),
                },
            },
        ];

        let rendered = runtime
            .render_trace(&document)
            .expect("render should succeed");
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
                op: SavedSyncOp::BootReason,
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
        let document: RunDocument = envelope.items;
        let insertion_index = document.len();

        let choices = runtime
            .insertion_choices(&document, insertion_index)
            .expect("choices should load");
        assert!(
            !choices.is_empty(),
            "expected choices at end of simulator1 trace"
        );

        let target = FormTarget::Insert { insertion_index };
        let complete = choices
            .iter()
            .enumerate()
            .filter_map(|(choice_index, choice)| {
                let spec = runtime
                    .form_spec(&document, &target, choice_index)
                    .expect("form spec should load");
                let state = runtime
                    .initial_form_state(&document, &target, choice_index)
                    .expect("form state should load");
                simulator::editor::form_is_auto_acceptable(&spec, &state)
                    .then_some(choice.label.clone())
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

        let session = open_trace::<InfoPanelSimulatorRuntime>(&path, 120, 30).expect("open trace");
        let rendered = runtime
            .render_trace(&session.state.view.trace)
            .expect("rendered rows");
        let step_count = step_count(&rendered.rows);
        let previous_step = step_count - 2;
        let last_step = step_count - 1;

        let mut previous_session =
            open_trace::<InfoPanelSimulatorRuntime>(&path, 120, 30).expect("reopen trace");
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
