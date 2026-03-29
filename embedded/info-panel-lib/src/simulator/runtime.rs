use super::forms::*;
use super::replay::*;
use super::saved::*;
use super::*;
use simulator::editor::RenderedTrace;

const TRIVIAL_PREVIEW_LIMIT: usize = 8;

pub struct InfoPanelSimulatorRuntime;

impl InfoPanelSimulatorRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl TraceRuntime for InfoPanelSimulatorRuntime {
    fn render_trace(&self, document: &RunDocument) -> Result<RenderedTrace, String> {
        let items = parse_items(document)?;
        let snapshot = replay_items(&items)?;
        Ok(RenderedTrace {
            rows: snapshot.rows,
            replay_error: snapshot.replay_error,
        })
    }

    fn preview_trivial_chain(
        &self,
        document: &RunDocument,
        insertion_index: usize,
    ) -> Result<Vec<String>, String> {
        let items = parse_items(document)?;
        if insertion_index > items.len() {
            return Err(format!("invalid insertion index {insertion_index}"));
        }
        preview_trivial_chain_from_prefix(&items[..insertion_index], TRIVIAL_PREVIEW_LIMIT)
    }

    fn apply_trivial_chain(
        &self,
        document: &mut RunDocument,
        insertion_index: usize,
    ) -> Result<usize, String> {
        let mut items = parse_items(document)?;
        if insertion_index > items.len() {
            return Err(format!("invalid insertion index {insertion_index}"));
        }
        let (new_items, _) = trivial_chain_from_prefix(&items[..insertion_index])?;
        let count = new_items.len();
        for (offset, item) in new_items.into_iter().enumerate() {
            items.insert(insertion_index + offset, item);
        }
        document.items = items_to_json(items)?;
        Ok(count)
    }

    fn insertion_choices(
        &self,
        document: &RunDocument,
        insertion_index: usize,
    ) -> Result<Vec<InsertionChoice>, String> {
        let items = parse_items(document)?;
        if insertion_index > items.len() {
            return Err(format!("invalid insertion index {insertion_index}"));
        }
        let snapshot = replay_items(&items[..insertion_index])?;
        Ok(snapshot
            .possible
            .iter()
            .map(|event| InsertionChoice {
                label: format_possible_event(event),
            })
            .collect())
    }

    fn begin_insert_form(
        &self,
        document: &RunDocument,
        insertion_index: usize,
        choice_index: usize,
    ) -> Result<Box<dyn FormController>, String> {
        let items = parse_items(document)?;
        if insertion_index > items.len() {
            return Err(format!("invalid insertion index {insertion_index}"));
        }
        let snapshot = replay_items(&items[..insertion_index])?;
        let Some(choice) = snapshot.possible.get(choice_index) else {
            return Err(format!("invalid choice index {choice_index}"));
        };
        let new_items = choice_to_saved_items(
            &snapshot.used_ids,
            &snapshot.runtime_to_symbolic,
            snapshot.current_ticks,
            choice,
        )?;
        let store_read_text = match new_items.as_slice() {
            [SavedItem::OutboundCreateSync {
                op: SavedSyncOp::StoreRead { .. },
                ..
            }, SavedItem::InboundReturnSync {
                result: SavedSyncResult::StoreReadOk { values },
                ..
            }] => Some(format_store_read_text(values)),
            _ => None,
        };
        Ok(form_for_items(
            "Insert event",
            new_items,
            None,
            store_read_text,
            false,
            None,
            false,
        ))
    }

    fn edit_choices(
        &self,
        document: &RunDocument,
        item_index: usize,
    ) -> Result<Vec<InsertionChoice>, String> {
        let items = parse_items(document)?;
        let (start, end) = removal_span(&items, item_index)?;
        let mut reduced = items.clone();
        reduced.drain(start..end);
        let snapshot = replay_items(&reduced[..start])?;
        Ok(snapshot
            .possible
            .iter()
            .map(|event| InsertionChoice {
                label: format_possible_event(event),
            })
            .collect())
    }

    fn begin_edit_form(
        &self,
        document: &RunDocument,
        item_index: usize,
        choice_index: usize,
    ) -> Result<Box<dyn FormController>, String> {
        let items = parse_items(document)?;
        let (start, end) = removal_span(&items, item_index)?;
        let mut reduced = items.clone();
        reduced.drain(start..end);
        let snapshot = replay_items(&reduced[..start])?;
        let Some(choice) = snapshot.possible.get(choice_index) else {
            return Err(format!("invalid choice index {choice_index}"));
        };
        let new_items = choice_to_saved_items(
            &snapshot.used_ids,
            &snapshot.runtime_to_symbolic,
            snapshot.current_ticks,
            choice,
        )?;
        Ok(form_for_items(
            "Edit event",
            new_items,
            Some(current_boot_reason_selection(document, item_index)),
            Some(current_store_read_text(document, item_index)),
            current_store_read_error_mode(document, item_index),
            Some(current_store_unit_text(document, item_index)),
            current_store_unit_error_mode(document, item_index),
        ))
    }

    fn apply_form(
        &self,
        document: &mut RunDocument,
        target: &FormTarget,
        items: Vec<serde_json::Value>,
    ) -> Result<(), String> {
        let mut saved_items = parse_items(document)?;
        match target {
            FormTarget::Insert { insertion_index } => {
                if *insertion_index > saved_items.len() {
                    return Err(format!("invalid insertion index {insertion_index}"));
                }
                for (offset, item) in items.into_iter().enumerate() {
                    let saved: SavedItem =
                        serde_json::from_value(item).map_err(|err| err.to_string())?;
                    saved_items.insert(insertion_index + offset, saved);
                }
            }
            FormTarget::Edit { item_index } => {
                let (start, end) = removal_span(&saved_items, *item_index)?;
                let replacements = items
                    .into_iter()
                    .map(|item| serde_json::from_value(item).map_err(|err| err.to_string()))
                    .collect::<Result<Vec<SavedItem>, _>>()?;
                saved_items.splice(start..end, replacements);
            }
        }
        document.items = items_to_json(saved_items)?;
        Ok(())
    }

    fn delete_item(&self, document: &mut RunDocument, item_index: usize) -> Result<(), String> {
        let mut items = parse_items(document)?;
        let (start, end) = removal_span(&items, item_index)?;
        items.drain(start..end);
        document.items = items
            .into_iter()
            .map(|item| serde_json::to_value(item).map_err(|err| err.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
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
        document.items = items_to_json(items)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::types::{AsyncOp, AsyncResult, SyncOp};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn saved_items(document: &RunDocument) -> Vec<SavedItem> {
        parse_items(document).expect("document should parse")
    }

    #[test]
    fn inserting_choice_creates_symbolic_outbound_and_target() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let mut document = RunDocument::default();

        let _choices = runtime
            .insertion_choices(&document, 0)
            .expect("choices should load");
        let choice_index = 0;

        let mut form = runtime
            .begin_insert_form(&document, 0, choice_index)
            .expect("insert form should open");
        let FormResult::Save {
            items: saved_json_items,
        } = form
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("form save should succeed")
        else {
            panic!("expected form save");
        };
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

        let first = document.items[0]
            .as_object()
            .expect("first item should be object");
        assert_eq!(first.get("type").and_then(|v| v.as_str()), Some("outbound"));
        assert!(matches!(
            first.get("event_type").and_then(|v| v.as_str()),
            Some("create_sync") | Some("create_async")
        ));
        let second = document.items[1]
            .as_object()
            .expect("second item should be object");
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
        let mut document = RunDocument::default();
        let choices = runtime
            .insertion_choices(&document, 0)
            .expect("choices should load");
        let choice_index = choices
            .iter()
            .position(|choice| {
                choice.label.starts_with("ResolveAsync") || choice.label.starts_with("ReturnSync")
            })
            .expect("expected a completion choice");
        let mut form = runtime
            .begin_insert_form(&document, 0, choice_index)
            .expect("insert form should open");
        let FormResult::Save {
            items: saved_json_items,
        } = form
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("form save should succeed")
        else {
            panic!("expected form save");
        };
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
        let mut form = form_for_items(
            "Edit event",
            vec![
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
            ],
            Some(0),
            None,
            false,
            None,
            false,
        );
        let _ = form
            .handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .expect("move should succeed");
        let FormResult::Save { items } = form
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("save should succeed")
        else {
            panic!("expected save");
        };

        let items = items
            .into_iter()
            .map(|item| serde_json::from_value(item).expect("saved item should parse"))
            .collect::<Vec<SavedItem>>();
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
        let mut form = form_for_items(
            "Edit event",
            vec![
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
            ],
            None,
            Some("ssid=new\npw=updated".to_string()),
            false,
            None,
            false,
        );

        let FormResult::Save { items } = form
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("save should succeed")
        else {
            panic!("expected save");
        };

        let items = items
            .into_iter()
            .map(|item| serde_json::from_value(item).expect("saved item should parse"))
            .collect::<Vec<SavedItem>>();
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
        let mut form = form_for_items(
            "Edit event",
            vec![
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
            ],
            None,
            Some("nvs failed".to_string()),
            false,
            None,
            false,
        );

        let _ = form
            .handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE))
            .expect("toggle should succeed");
        let FormResult::Save { items } = form
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("save should succeed")
        else {
            panic!("expected save");
        };

        let items = items
            .into_iter()
            .map(|item| serde_json::from_value(item).expect("saved item should parse"))
            .collect::<Vec<SavedItem>>();
        assert!(matches!(
            &items[1],
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::StoreReadErr { message },
                ..
            } if message == "nvs failed"
        ));
    }

    #[test]
    fn tft_sync_form_can_return_error() {
        let mut form = form_for_items(
            "Edit event",
            vec![
                SavedItem::OutboundCreateSync {
                    id: "tft_set_rst_high".to_string(),
                    op: SavedSyncOp::TftSetRstHigh,
                },
                SavedItem::InboundReturnSync {
                    target: "tft_set_rst_high".to_string(),
                    result: SavedSyncResult::UnitOk,
                },
            ],
            None,
            None,
            false,
            Some("spi timeout".to_string()),
            false,
        );

        let _ = form
            .handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE))
            .expect("toggle should succeed");
        let FormResult::Save { items } = form
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("save should succeed")
        else {
            panic!("expected save");
        };

        let items = items
            .into_iter()
            .map(|item| serde_json::from_value(item).expect("saved item should parse"))
            .collect::<Vec<SavedItem>>();
        assert!(matches!(
            &items[1],
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::UnitErr { message },
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
    }

    #[test]
    fn invalid_trace_still_renders_and_marks_invalid_rows() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let document = RunDocument {
            kind: simulator::editor::SIMULATOR_RUN_KIND.to_string(),
            version: simulator::editor::SIMULATOR_RUN_VERSION,
            items: items_to_json(vec![SavedItem::OutboundCreateAsync {
                id: "wifi_disconnect_2".to_string(),
                op: SavedAsyncOp::WifiDisconnect,
            }])
            .unwrap(),
        };

        let rendered = runtime
            .render_trace(&document)
            .expect("render should succeed");
        assert!(rendered.replay_error.is_some());
        assert!(rendered.rows.iter().any(|row| row.is_invalid));
    }

    #[test]
    fn invalid_inbound_item_still_renders_as_invalid_suffix() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let document = RunDocument {
            kind: simulator::editor::SIMULATOR_RUN_KIND.to_string(),
            version: simulator::editor::SIMULATOR_RUN_VERSION,
            items: items_to_json(vec![
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
            ])
            .unwrap(),
        };

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
        let mut document = RunDocument {
            kind: simulator::editor::SIMULATOR_RUN_KIND.to_string(),
            version: simulator::editor::SIMULATOR_RUN_VERSION,
            items: items_to_json(vec![SavedItem::OutboundCreateAsync {
                id: "wifi_disconnect_2".to_string(),
                op: SavedAsyncOp::WifiDisconnect,
            }])
            .unwrap(),
        };

        runtime
            .delete_item(&mut document, 0)
            .expect("delete should succeed");
        assert!(document.items.is_empty());
    }

    #[test]
    fn trace_after_boot_reason_includes_tft_events() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let document = RunDocument {
            kind: simulator::editor::SIMULATOR_RUN_KIND.to_string(),
            version: simulator::editor::SIMULATOR_RUN_VERSION,
            items: items_to_json(vec![
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
            ])
            .unwrap(),
        };

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
    fn trivial_chain_preview_and_apply_progresses_tft_init() {
        let runtime = InfoPanelSimulatorRuntime::new();
        let mut document = RunDocument {
            kind: simulator::editor::SIMULATOR_RUN_KIND.to_string(),
            version: simulator::editor::SIMULATOR_RUN_VERSION,
            items: items_to_json(vec![
                SavedItem::OutboundCreateSync {
                    id: "tft_set_rst_high".to_string(),
                    op: SavedSyncOp::TftSetRstHigh,
                },
                SavedItem::InboundReturnSync {
                    target: "tft_set_rst_high".to_string(),
                    result: SavedSyncResult::UnitOk,
                },
                SavedItem::OutboundCreateAsync {
                    id: "sleep_20ms".to_string(),
                    op: SavedAsyncOp::Sleep { duration_ms: 20 },
                },
                SavedItem::InboundResolveAsync {
                    target: "sleep_20ms".to_string(),
                    result: SavedAsyncResult::SleepDone,
                },
            ])
            .unwrap(),
        };

        let preview = runtime
            .preview_trivial_chain(&document, 4)
            .expect("preview should succeed");
        assert!(!preview.is_empty());
        assert!(preview
            .iter()
            .any(|label| label.contains("ReturnSync#") || label.contains("ResolveAsync#")));

        let inserted = runtime
            .apply_trivial_chain(&mut document, 4)
            .expect("apply should succeed");
        assert!(inserted > 0);
        assert!(document.items.len() > 2);
    }
}
