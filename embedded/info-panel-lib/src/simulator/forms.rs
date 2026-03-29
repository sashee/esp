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
                    result: SavedSyncResult::StoreRead {
                        values: parse_store_read_text(keys, text)?,
                    },
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
                text,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "namespace: {namespace}\nkeys: {}\n\n{}\n\nType key=value lines. enter: save  esc: cancel",
                        keys.join(", "),
                        text
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
            InfoPanelForm::StoreRead { text, .. } => match key.code {
                KeyCode::Enter => Ok(FormResult::Save {
                    items: self.to_json_items()?,
                }),
                KeyCode::Esc => Ok(FormResult::Cancel),
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
                result: SavedSyncResult::StoreRead { values },
                ..
            } => Some(format_store_read_text(&values)),
            _ => None,
        })
        .unwrap_or_default()
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
            result: SavedSyncResult::StoreRead { .. },
            ..
        }] => Box::new(InfoPanelForm::StoreRead {
            title: title.to_string(),
            target: id.clone(),
            namespace: namespace.clone(),
            keys: keys.clone(),
            text: store_read_text.unwrap_or_default(),
            outbound: Some(items[0].clone()),
        }),
        [SavedItem::InboundReturnSync {
            target: id,
            result: SavedSyncResult::StoreRead { .. },
        }] => Box::new(InfoPanelForm::StoreRead {
            title: title.to_string(),
            target: id.clone(),
            namespace: String::new(),
            keys: Vec::new(),
            text: store_read_text.unwrap_or_default(),
            outbound: None,
        }),
        _ => Box::new(InfoPanelForm::Confirm {
            title: title.to_string(),
            items,
        }),
    }
}
