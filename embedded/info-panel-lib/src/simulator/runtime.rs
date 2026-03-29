use super::forms::*;
use super::replay::*;
use super::saved::*;
use super::*;
use simulator::editor::RenderedTrace;

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
        let new_items =
            choice_to_saved_items(&snapshot.used_ids, &snapshot.runtime_to_symbolic, choice)?;
        let store_read_text = match new_items.as_slice() {
            [SavedItem::OutboundCreateSync {
                op: SavedSyncOp::StoreRead { .. },
                ..
            }, SavedItem::InboundReturnSync {
                result: SavedSyncResult::StoreRead { values },
                ..
            }] => Some(format_store_read_text(values)),
            _ => None,
        };
        Ok(form_for_items(
            "Insert event",
            new_items,
            None,
            store_read_text,
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
        let new_items =
            choice_to_saved_items(&snapshot.used_ids, &snapshot.runtime_to_symbolic, choice)?;
        Ok(form_for_items(
            "Edit event",
            new_items,
            Some(current_boot_reason_selection(document, item_index)),
            Some(current_store_read_text(document, item_index)),
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    result: SavedSyncResult::StoreRead {
                        values: BTreeMap::from([
                            ("ssid".to_string(), "old".to_string()),
                            ("pw".to_string(), "secret".to_string()),
                        ]),
                    },
                },
            ],
            None,
            Some("ssid=new\npw=updated".to_string()),
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
                result: SavedSyncResult::StoreRead { values },
                ..
            } if values.get("ssid") == Some(&"new".to_string()) && values.get("pw") == Some(&"updated".to_string())
        ));
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
}
