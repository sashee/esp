use super::replay::*;
use super::saved::*;
use super::*;

pub(super) enum InfoPanelForm {
    Confirm {
        title: String,
        items: Vec<SavedItem>,
    },
    BootReason {
        title: String,
        target: String,
        selected: usize,
        outbound: Option<SavedItem>,
    },
    StoreRead {
        title: String,
        target: String,
        namespace: String,
        keys: Vec<String>,
        error_mode: bool,
        text: String,
        outbound: Option<SavedItem>,
    },
    SyncUnit {
        title: String,
        target: String,
        operation_label: String,
        error_mode: bool,
        text: String,
        outbound: Option<SavedItem>,
    },
}

impl InfoPanelForm {
    fn to_json_items(&self) -> Result<Vec<serde_json::Value>, String> {
        match self {
            InfoPanelForm::Confirm { items, .. } => items_to_json(items.clone()),
            InfoPanelForm::BootReason {
                target,
                selected,
                outbound,
                ..
            } => {
                let mut items = Vec::new();
                if let Some(outbound) = outbound.clone() {
                    items.push(outbound);
                }
                items.push(SavedItem::InboundReturnSync {
                    target: target.clone(),
                    result: SavedSyncResult::BootReason {
                        value: BOOT_REASON_OPTIONS[*selected].to_string(),
                    },
                });
                items_to_json(items)
            }
            InfoPanelForm::StoreRead {
                target,
                namespace: _,
                keys,
                error_mode,
                text,
                outbound,
                ..
            } => {
                let mut items = Vec::new();
                if let Some(outbound) = outbound.clone() {
                    items.push(outbound);
                }
                items.push(SavedItem::InboundReturnSync {
                    target: target.clone(),
                    result: if *error_mode {
                        SavedSyncResult::StoreReadErr {
                            message: text.trim().to_string(),
                        }
                    } else {
                        SavedSyncResult::StoreReadOk {
                            values: parse_store_read_text(keys, text)?,
                        }
                    },
                });
                items_to_json(items)
            }
            InfoPanelForm::SyncUnit {
                target,
                operation_label: _,
                error_mode,
                text,
                outbound,
                ..
            } => {
                let mut items = Vec::new();
                if let Some(outbound) = outbound.clone() {
                    items.push(outbound);
                }
                let result = if *error_mode {
                    SavedSyncResult::UnitErr {
                        message: text.trim().to_string(),
                    }
                } else {
                    SavedSyncResult::UnitOk
                };
                items.push(SavedItem::InboundReturnSync {
                    target: target.clone(),
                    result,
                });
                items_to_json(items)
            }
        }
    }
}

impl FormController for InfoPanelForm {
    fn render(&self, frame: &mut simulator::ratatui::Frame<'_>, area: Rect) {
        match self {
            InfoPanelForm::Confirm { title, items } => {
                let lines = items
                    .iter()
                    .map(format_saved_item)
                    .collect::<Vec<_>>()
                    .join("\n");
                frame.render_widget(
                    Paragraph::new(format!("{lines}\n\nenter: save  esc: cancel"))
                        .block(Block::default().title(title.as_str()).borders(Borders::ALL)),
                    area,
                );
            }
            InfoPanelForm::BootReason {
                title, selected, ..
            } => {
                let items = BOOT_REASON_OPTIONS
                    .iter()
                    .map(|value| ListItem::new((*value).to_string()))
                    .collect::<Vec<_>>();
                let mut state = ListState::default();
                state.select(Some(*selected));
                let widget = List::new(items)
                    .block(
                        Block::default()
                            .title(format!("{} (enter: save  esc: cancel)", title))
                            .borders(Borders::ALL),
                    )
                    .highlight_symbol("> ")
                    .highlight_spacing(simulator::ratatui::widgets::HighlightSpacing::Always)
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
                frame.render_stateful_widget(widget, area, &mut state);
            }
            InfoPanelForm::StoreRead {
                title,
                namespace,
                keys,
                error_mode,
                text,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "mode: {} (press m to toggle)\nnamespace: {namespace}\nkeys: {}\n\n{}\n\n{}\nenter: save  esc: cancel",
                        if *error_mode { "error" } else { "success" },
                        keys.join(", "),
                        text,
                        if *error_mode {
                            "Type an error message."
                        } else {
                            "Type key=value lines. Press Tab for newline."
                        }
                    ))
                    .block(Block::default().title(title.as_str()).borders(Borders::ALL)),
                    area,
                );
            }
            InfoPanelForm::SyncUnit {
                title,
                operation_label,
                error_mode,
                text,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "mode: {} (press m to toggle)\noperation: {}\n\n{}\n\n{}\nenter: save  esc: cancel",
                        if *error_mode { "error" } else { "success" },
                        operation_label,
                        text,
                        if *error_mode {
                            "Type an error message."
                        } else {
                            "Success returns immediately. Optional text is ignored."
                        }
                    ))
                    .block(Block::default().title(title.as_str()).borders(Borders::ALL)),
                    area,
                );
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<FormResult, String> {
        match self {
            InfoPanelForm::Confirm { .. } => match key.code {
                KeyCode::Enter => Ok(FormResult::Save {
                    items: self.to_json_items()?,
                }),
                KeyCode::Esc => Ok(FormResult::Cancel),
                _ => Ok(FormResult::Continue),
            },
            InfoPanelForm::BootReason { selected, .. } => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                    Ok(FormResult::Continue)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if *selected + 1 < BOOT_REASON_OPTIONS.len() {
                        *selected += 1;
                    }
                    Ok(FormResult::Continue)
                }
                KeyCode::Enter => Ok(FormResult::Save {
                    items: self.to_json_items()?,
                }),
                KeyCode::Esc => Ok(FormResult::Cancel),
                _ => Ok(FormResult::Continue),
            },
            InfoPanelForm::StoreRead {
                text, error_mode, ..
            }
            | InfoPanelForm::SyncUnit {
                text, error_mode, ..
            } => match key.code {
                KeyCode::Enter => Ok(FormResult::Save {
                    items: self.to_json_items()?,
                }),
                KeyCode::Esc => Ok(FormResult::Cancel),
                KeyCode::Char('m') => {
                    *error_mode = !*error_mode;
                    Ok(FormResult::Continue)
                }
                KeyCode::Backspace => {
                    text.pop();
                    Ok(FormResult::Continue)
                }
                KeyCode::Char(ch) => {
                    text.push(ch);
                    Ok(FormResult::Continue)
                }
                KeyCode::Tab => {
                    text.push('\n');
                    Ok(FormResult::Continue)
                }
                _ => Ok(FormResult::Continue),
            },
        }
    }
}

pub(super) fn items_to_json(items: Vec<SavedItem>) -> Result<Vec<serde_json::Value>, String> {
    items
        .into_iter()
        .map(|item| serde_json::to_value(item).map_err(|err| err.to_string()))
        .collect()
}

pub(super) fn current_boot_reason_selection(document: &RunDocument, item_index: usize) -> usize {
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

pub(super) fn current_store_read_text(document: &RunDocument, item_index: usize) -> String {
    parse_items(document)
        .ok()
        .and_then(|items| items.get(item_index).cloned())
        .and_then(|item| match item {
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::StoreReadOk { values },
                ..
            } => Some(format_store_read_text(&values)),
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::StoreReadErr { message },
                ..
            } => Some(message),
            _ => None,
        })
        .unwrap_or_default()
}

pub(super) fn current_store_read_error_mode(document: &RunDocument, item_index: usize) -> bool {
    parse_items(document)
        .ok()
        .and_then(|items| items.get(item_index).cloned())
        .is_some_and(|item| {
            matches!(
                item,
                SavedItem::InboundReturnSync {
                    result: SavedSyncResult::StoreReadErr { .. },
                    ..
                }
            )
        })
}

pub(super) fn current_store_unit_text(document: &RunDocument, item_index: usize) -> String {
    parse_items(document)
        .ok()
        .and_then(|items| items.get(item_index).cloned())
        .and_then(|item| match item {
            SavedItem::InboundReturnSync {
                result: SavedSyncResult::UnitErr { message },
                ..
            } => Some(message),
            _ => None,
        })
        .unwrap_or_default()
}

pub(super) fn current_store_unit_error_mode(document: &RunDocument, item_index: usize) -> bool {
    parse_items(document)
        .ok()
        .and_then(|items| items.get(item_index).cloned())
        .is_some_and(|item| {
            matches!(
                item,
                SavedItem::InboundReturnSync {
                    result: SavedSyncResult::UnitErr { .. },
                    ..
                }
            )
        })
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

pub(super) fn form_for_items(
    title: &str,
    items: Vec<SavedItem>,
    boot_reason_selection: Option<usize>,
    store_read_text: Option<String>,
    store_read_error_mode: bool,
    store_unit_text: Option<String>,
    store_unit_error_mode: bool,
) -> Box<dyn FormController> {
    match items.as_slice() {
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
        }] => Box::new(InfoPanelForm::BootReason {
            title: title.to_string(),
            target: id.clone(),
            selected: boot_reason_selection.unwrap_or(0),
            outbound: if matches!(items.first(), Some(SavedItem::OutboundCreateSync { .. })) {
                Some(items[0].clone())
            } else {
                None
            },
        }),
        [SavedItem::OutboundCreateSync {
            id,
            op: SavedSyncOp::StoreRead { namespace, keys },
        }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::StoreReadOk { .. } | SavedSyncResult::StoreReadErr { .. },
            ..
        }] => Box::new(InfoPanelForm::StoreRead {
            title: title.to_string(),
            target: id.clone(),
            namespace: namespace.clone(),
            keys: keys.clone(),
            error_mode: store_read_error_mode,
            text: store_read_text.unwrap_or_default(),
            outbound: Some(items[0].clone()),
        }),
        [SavedItem::InboundReturnSync {
            target: id,
            result: SavedSyncResult::StoreReadOk { .. } | SavedSyncResult::StoreReadErr { .. },
        }] => Box::new(InfoPanelForm::StoreRead {
            title: title.to_string(),
            target: id.clone(),
            namespace: String::new(),
            keys: Vec::new(),
            error_mode: store_read_error_mode,
            text: store_read_text.unwrap_or_default(),
            outbound: None,
        }),
        [SavedItem::OutboundCreateSync { id, op }, SavedItem::InboundReturnSync {
            result: SavedSyncResult::UnitOk | SavedSyncResult::UnitErr { .. },
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
            Box::new(InfoPanelForm::SyncUnit {
                title: title.to_string(),
                target: id.clone(),
                operation_label: format_saved_sync_op(op),
                error_mode: store_unit_error_mode,
                text: store_unit_text.unwrap_or_default(),
                outbound: Some(items[0].clone()),
            })
        }
        _ => Box::new(InfoPanelForm::Confirm {
            title: title.to_string(),
            items,
        }),
    }
}
