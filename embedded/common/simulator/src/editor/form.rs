use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormSpec {
    pub title: String,
    pub details: Vec<String>,
    pub fields: Vec<FormField>,
    pub auto_accept_if_complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub kind: FormFieldKind,
    pub help: Option<String>,
    pub initial_value: Option<FormValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormFieldKind {
    Text {
        multiline: bool,
    },
    Select {
        options: Vec<String>,
    },
    Toggle {
        false_label: String,
        true_label: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormValue {
    Text(String),
    Select(usize),
    Toggle(bool),
}

pub type FormState = BTreeMap<String, FormValue>;

pub fn form_state_from_spec(spec: &FormSpec) -> FormState {
    spec.fields
        .iter()
        .filter_map(|field| {
            field
                .initial_value
                .as_ref()
                .map(|value| (field.id.clone(), value.clone()))
        })
        .collect()
}

pub fn missing_form_fields(spec: &FormSpec, state: &FormState) -> Vec<String> {
    spec.fields
        .iter()
        .filter(|field| !state.contains_key(&field.id))
        .map(|field| field.label.clone())
        .collect()
}

pub fn form_is_complete(spec: &FormSpec, state: &FormState) -> bool {
    missing_form_fields(spec, state).is_empty()
}

pub fn form_is_auto_acceptable(spec: &FormSpec, state: &FormState) -> bool {
    spec.auto_accept_if_complete && form_is_complete(spec, state)
}
