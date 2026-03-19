#[cfg(target_os = "espidf")]
pub mod esp_idf;

#[cfg(test)]
mod tests;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use core::fmt::Write as _;
use core::future::Future;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_futures::select::{select3, Either3};
use embassy_time::{Duration, Instant};
use log::{error, info, warn};
use std::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};


#[derive(Clone, Debug)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[async_trait]
pub trait SelectOptions: Send + Sync {
    async fn options(&self) -> Vec<SelectOption>;
}

#[derive(Clone)]
pub enum FieldKind {
    Text,
    Password,
    Number { min: i64, max: i64 },
    Select { options: Arc<dyn SelectOptions> },
}

#[derive(Clone)]
pub struct FieldSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    pub required: bool,
}

impl FieldSpec {
    pub const fn text(key: &'static str, label: &'static str) -> Self {
        Self {
            key,
            label,
            kind: FieldKind::Text,
            required: true,
        }
    }

    pub const fn password(key: &'static str, label: &'static str) -> Self {
        Self {
            key,
            label,
            kind: FieldKind::Password,
            required: true,
        }
    }

    pub const fn number(key: &'static str, label: &'static str, min: i64, max: i64) -> Self {
        Self {
            key,
            label,
            kind: FieldKind::Number { min, max },
            required: true,
        }
    }

    pub fn select(key: &'static str, label: &'static str, options: impl SelectOptions + 'static) -> Self {
        Self {
            key,
            label,
            kind: FieldKind::Select { options: Arc::new(options) },
            required: true,
        }
    }
}

#[derive(Clone)]
pub struct ConfigSpec {
    pub namespace: &'static str,
    pub ap_prefix: &'static str,
    pub title: &'static str,
    pub fields: Vec<FieldSpec>,
}

#[derive(Clone, Debug, Default)]
pub struct StoredConfig {
    values: BTreeMap<String, String>,
}

impl StoredConfig {
    pub fn new(values: BTreeMap<String, String>) -> Self {
        Self { values }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn into_map(self) -> BTreeMap<String, String> {
        self.values
    }
}

#[derive(Clone, Debug)]
pub enum ConfigState {
    Missing,
    Ready(StoredConfig),
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigTiming {
    pub idle_timeout: Duration,
    pub connected_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IpConfig {
    pub ip: String,
    pub gateway: String,
    pub netmask: String,
}

impl IpConfig {
    pub fn new(
        ip: impl Into<String>,
        gateway: impl Into<String>,
        netmask: impl Into<String>,
    ) -> Self {
        Self {
            ip: ip.into(),
            gateway: gateway.into(),
            netmask: netmask.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPointConfig {
    pub ssid: String,
    pub channel: u8,
    pub max_connections: u8,
    pub ip_config: IpConfig,
}

impl AccessPointConfig {
    pub fn new(ssid: impl Into<String>, ip_config: IpConfig) -> Self {
        Self {
            ssid: ssid.into(),
            channel: 1,
            max_connections: 1,
            ip_config,
        }
    }
}

#[allow(async_fn_in_trait)]
pub trait AccessPointClientConnectedSubscription {
    async fn next(&mut self) -> Result<()>;
}

#[allow(async_fn_in_trait)]
pub trait AccessPointStoppedSubscription {
    async fn next(&mut self) -> Result<()>;
}

pub trait ConfigStore {
    fn read(&self, namespace: &str, keys: &[&str]) -> Result<BTreeMap<String, String>>;

    fn write(&self, namespace: &str, values: &BTreeMap<String, String>) -> Result<()>;

    fn remove(&self, namespace: &str, keys: &[&str]) -> Result<()>;
}

pub trait ConfigPlatform {
    fn mac_address(&self) -> Result<[u8; 6]>;

    fn reboot(&self) -> !;
}

#[allow(async_fn_in_trait)]
pub trait ConfigClock {
    fn now(&self) -> Instant;

    async fn sleep(&self, duration: Duration);
}

#[allow(async_fn_in_trait)]
pub trait ConfigWifi {
    type AccessPointClientConnectedSubscription: AccessPointClientConnectedSubscription;
    type AccessPointStoppedSubscription: AccessPointStoppedSubscription;

    async fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<IpConfig>;

    async fn stop_access_point(&mut self) -> Result<()>;

    fn subscribe_access_point_client_connected(
        &self,
    ) -> Result<Self::AccessPointClientConnectedSubscription>;

    fn subscribe_access_point_stopped(&self) -> Result<Self::AccessPointStoppedSubscription>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Other(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpEndpoint {
    pub method: &'static str,
    pub path: &'static str,
}

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status_code: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

pub trait ConfigHttpBackend {
    type Server;

    fn start<H, Fut>(self, endpoints: &'static [HttpEndpoint], handler: H) -> Result<Self::Server>
    where
        H: Fn(HttpRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HttpResponse>> + Send;
}

const CONFIG_HTTP_ENDPOINTS: &[HttpEndpoint] = &[
    HttpEndpoint {
        method: "GET",
        path: "/",
    },
    HttpEndpoint {
        method: "POST",
        path: "/save",
    },
    HttpEndpoint {
        method: "POST",
        path: "/reset",
    },
];

pub async fn enter_config_mode<W, S, H, P, C>(
    spec: ConfigSpec,
    reason: &str,
    wifi: &mut W,
    store: S,
    http: H,
    platform: P,
    clock: C,
    timing: ConfigTiming,
) -> Result<()>
where
    W: ConfigWifi,
    S: ConfigStore + Clone + Send + Sync + 'static,
    H: ConfigHttpBackend,
    P: ConfigPlatform,
    C: ConfigClock,
{
    error!("entering config portal: {reason}");

    let ap_ssid = ap_ssid(spec.ap_prefix, platform.mac_address()?);
    start_access_point(wifi, &ap_ssid).await?;
    let activity = Arc::new(ConfigActivity::default());
    let reason = reason.to_string();
    let http_activity = activity.clone();
    let server = http.start(CONFIG_HTTP_ENDPOINTS, move |request| {
        let reason = reason.to_string();
        let store = store.clone();
        let http_activity = http_activity.clone();
        let spec = spec.clone();
        async move {
            handle_http_request(&spec, &reason, &store, &http_activity, request).await
        }
    })?;

    info!("config portal ready on SSID {ap_ssid}");
    let _server = server;

    let started_at = clock.now();
    let mut client_connected = false;
    let mut client_connected_events = wifi.subscribe_access_point_client_connected()?;
    let mut stopped_events = wifi.subscribe_access_point_stopped()?;

    loop {
        let elapsed = clock.now() - started_at;
        let limit = if client_connected {
            timing.connected_timeout
        } else {
            timing.idle_timeout
        };

        if elapsed >= limit {
            warn!("config portal timed out after {:?}", elapsed);
            stop_access_point(wifi).await?;
            return Ok(());
        }

        let remaining = limit - elapsed;
        let wait_step = if remaining > Duration::from_millis(250) {
            Duration::from_millis(250)
        } else {
            remaining
        };

        match select3(
            client_connected_events.next(),
            stopped_events.next(),
            clock.sleep(wait_step),
        )
        .await
        {
            Either3::First(result) => {
                result?;
                if !client_connected {
                    client_connected = true;
                    info!("config portal station connected");
                }
            }
            Either3::Second(result) => {
                result?;
                bail!("softap stopped unexpectedly");
            }
            Either3::Third(_) => {}
        }

        if activity.reboot_requested.load(Ordering::Relaxed) {
            platform.reboot();
        }
    }
}

pub fn read_config<S>(spec: &ConfigSpec, store: &S) -> Result<ConfigState>
where
    S: ConfigStore,
{
    let stored = store.read(spec.namespace, &field_keys(spec))?;

    let mut values = BTreeMap::new();
    for field in &spec.fields {
        let Some(value) = stored.get(field.key) else {
            return Ok(ConfigState::Missing);
        };
        values.insert(field.key.to_string(), value.clone());
    }

    Ok(ConfigState::Ready(StoredConfig { values }))
}

pub fn clear_config<S>(spec: &ConfigSpec, store: &S) -> Result<()>
where
    S: ConfigStore,
{
    store.remove(spec.namespace, &field_keys(spec))
}

pub fn save_config<S>(
    spec: &ConfigSpec,
    store: &S,
    submitted: &BTreeMap<String, String>,
) -> Result<StoredConfig>
where
    S: ConfigStore,
{
    let previous = read_existing_config(spec, store)?;
    validate_submitted(spec, submitted, &previous)?;

    let mut saved = BTreeMap::new();
    for field in &spec.fields {
        let value = match field.kind {
            FieldKind::Password => submitted
                .get(field.key)
                .and_then(|value| {
                    if value.is_empty() {
                        previous.get(field.key).map(ToString::to_string)
                    } else {
                        Some(value.clone())
                    }
                })
                .or_else(|| previous.get(field.key).map(ToString::to_string))
                .unwrap_or_default(),
            FieldKind::Text | FieldKind::Number { .. } => {
                submitted.get(field.key).cloned().unwrap_or_default()
            }
            FieldKind::Select { .. } => {
                let selected = submitted.get(field.key).cloned().unwrap_or_default();
                if selected == "__other__" {
                    submitted.get(&format!("{}_other", field.key)).cloned().unwrap_or_default()
                } else {
                    selected
                }
            }
        };
        saved.insert(field.key.to_string(), value);
    }

    store.write(spec.namespace, &saved)?;

    Ok(StoredConfig { values: saved })
}

pub fn ap_ssid(prefix: &str, mac: [u8; 6]) -> String {
    format!("{prefix}-{:02X}{:02X}", mac[4], mac[5])
}

#[derive(Default)]
struct ConfigActivity {
    reboot_requested: AtomicBool,
}

async fn start_access_point<W>(wifi: &mut W, ap_ssid: &str) -> Result<()>
where
    W: ConfigWifi,
{
    let ip_config = default_ap_ip_config();
    let mut ap = AccessPointConfig::new(ap_ssid, ip_config.clone());
    ap.channel = 1;
    ap.max_connections = 1;

    let started_ip_config = wifi.start_access_point(&ap).await?;
    info!("config portal SoftAP mode started");

    log_access_point_started(ap_ssid, &started_ip_config);

    Ok(())
}

async fn stop_access_point<W>(wifi: &mut W) -> Result<()>
where
    W: ConfigWifi,
{
    wifi.stop_access_point().await
}

async fn handle_http_request<S>(
    spec: &ConfigSpec,
    reason: &str,
    store: &S,
    activity: &ConfigActivity,
    request: HttpRequest,
) -> Result<HttpResponse>
where
    S: ConfigStore,
{
    info!(
        "config portal request: {} {}",
        request_method_name(&request.method),
        request.path
    );
    log_request_headers(&request.headers);

    match (&request.method, request.path.as_str()) {
        (HttpMethod::Get, "/") => {
            let state = read_config(spec, store)?;
            Ok(html_response(render_form(spec, reason, &state, None, None).await))
        }
        (HttpMethod::Post, "/save") => {
            let form = parse_request_form(&request)?;
            if let Err(err) = save_config(spec, store, &form) {
                let state = read_config(spec, store)?;
                return Ok(html_response(render_form(
                    spec,
                    reason,
                    &state,
                    Some(&form),
                    Some(&err.to_string()),
                ).await));
            }

            activity.reboot_requested.store(true, Ordering::Relaxed);
            Ok(html_response(success_page(
                "Saved configuration. Rebooting...",
            )))
        }
        (HttpMethod::Post, "/reset") => {
            clear_config(spec, store)?;
            activity.reboot_requested.store(true, Ordering::Relaxed);
            Ok(html_response(success_page(
                "Reset stored configuration. Rebooting...",
            )))
        }
        (HttpMethod::Other(_), "/")
        | (HttpMethod::Other(_), "/save")
        | (HttpMethod::Other(_), "/reset") => Ok(text_response(405, "Method not allowed")),
        (_, _) => Ok(text_response(404, "Not found")),
    }
}

async fn render_form(
    spec: &ConfigSpec,
    reason: &str,
    state: &ConfigState,
    submitted: Option<&BTreeMap<String, String>>,
    error_message: Option<&str>,
) -> String {
    let mut html = String::new();
    let _ = write!(
        html,
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>{}</style></head><body><main><h1>{}</h1><p>{}</p>",
        escape_html(spec.title),
        STYLE,
        escape_html(spec.title),
        escape_html(reason),
    );

    match state {
        ConfigState::Missing => html.push_str("<p class=\"note\">No stored configuration found.</p>"),
        ConfigState::Ready(_) => {}
    }

    if let Some(error_message) = error_message {
        let _ = write!(
            html,
            "<p class=\"error\">{}</p>",
            escape_html(error_message)
        );
    }

    html.push_str("<form method=\"post\" action=\"/save\">");
    for field in &spec.fields {
        let value = field_value(field, state, submitted);
        let has_stored_value = stored_field_value(state, field.key).is_some();
        
        match &field.kind {
            FieldKind::Text | FieldKind::Password | FieldKind::Number { .. } => {
                let input_type = match field.kind {
                    FieldKind::Text => "text",
                    FieldKind::Password => "password",
                    FieldKind::Number { .. } => "number",
                    FieldKind::Select { .. } => unreachable!(),
                };
                let required = if matches!(field.kind, FieldKind::Password) {
                    field.required && !has_stored_value
                } else {
                    field.required
                };
                let placeholder = if matches!(field.kind, FieldKind::Password) && has_stored_value {
                    "Leave blank to keep stored password"
                } else {
                    ""
                };
                let required_attr = if required { " required" } else { "" };
                let mut extra_attrs = String::new();
                if !placeholder.is_empty() {
                    let _ = write!(extra_attrs, " placeholder=\"{}\"", escape_html(placeholder));
                }
                let number_range = if let FieldKind::Number { min, max } = field.kind {
                    let _ = write!(extra_attrs, " min=\"{}\" max=\"{}\" step=\"1\"", min, max);
                    Some((min, max))
                } else {
                    None
                };

                if let Some((min, max)) = number_range {
                    let _ = write!(html, "<label><span>{}</span><p class=\"hint\">Allowed: {} - {}</p><input type=\"{}\" name=\"{}\" value=\"{}\" autocomplete=\"off\"{}{}></label>", escape_html(field.label), min, max, input_type, escape_html(field.key), escape_html(value), extra_attrs, required_attr);
                } else {
                    let _ = write!(
                        html,
                        "<label><span>{}</span><input type=\"{}\" name=\"{}\" value=\"{}\" autocomplete=\"off\"{}{}></label>",
                        escape_html(field.label),
                        input_type,
                        escape_html(field.key),
                        escape_html(value),
                        extra_attrs,
                        required_attr
                    );
                }
                
                if matches!(field.kind, FieldKind::Password) && has_stored_value {
                    html.push_str(
                        "<p class=\"hint\">A password is already stored; leave blank to keep it.</p>",
                    );
                }
            }
            FieldKind::Select { options } => {
                let required_attr = if field.required { " required" } else { "" };
                let options = options.options().await;
                
                let stored_value = stored_field_value(state, field.key);
                
                let submitted_value = submitted.and_then(|s| s.get(field.key).cloned());
                let submitted_other = submitted.and_then(|s| s.get(&format!("{}_other", field.key)).cloned());
                
                let selected_value = if let Some(ref sv) = submitted_value {
                    if sv == "__other__" {
                        submitted_other.as_deref().unwrap_or("")
                    } else {
                        sv.as_str()
                    }
                } else {
                    stored_value.unwrap_or("")
                };
                
                let value_is_in_options = options.iter().any(|o| o.value == selected_value);
                
                let _ = write!(
                    html,
                    "<label><span>{}</span><select name=\"{}\"{} onchange=\"var i=this.nextElementSibling;i.classList.toggle('hidden',this.value!=='__other__');i.required=this.value==='__other__'\">",
                    escape_html(field.key),
                    escape_html(field.key),
                    required_attr
                );
                
                for option in &options {
                    let selected = selected_value == option.value.as_str();
                    let _ = write!(
                        html,
                        "<option value=\"{}\"{}>{}</option>",
                        escape_html(&option.value),
                        if selected { " selected" } else { "" },
                        escape_html(&option.label)
                    );
                }
                
                let other_selected = submitted_value.as_deref() == Some("__other__") || 
                    (stored_value.is_some() && !value_is_in_options);
                let _ = write!(
                    html,
                    "<option value=\"__other__\"{}>Other...</option></select>",
                    if other_selected { " selected" } else { "" }
                );
                
                let other_text_value = if other_selected {
                    submitted_other.as_deref().unwrap_or(if !value_is_in_options { stored_value.unwrap_or("") } else { "" })
                } else {
                    ""
                };
                let hidden_attr = if other_selected { String::new() } else { r#" class="hidden""#.to_string() };
                let other_required = if other_selected { " required" } else { "" };
                let _ = write!(
                    html,
                    "<input type=\"text\" name=\"{}_other\" value=\"{}\"{} placeholder=\"Enter custom value\"{}>",
                    escape_html(field.key),
                    escape_html(other_text_value),
                    hidden_attr,
                    other_required
                );
                let _ = write!(html, "</label>");
            }
        }
    }
    html.push_str("<button type=\"submit\">Save and reboot</button></form>");
    html.push_str("<form method=\"post\" action=\"/reset\"><button class=\"danger\" type=\"submit\" onclick=\"return confirm('Are you sure? This will erase all stored configuration.')\">Reset stored config</button></form>");
    html.push_str("</main></body></html>");

    html
}

fn success_page(message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Config saved</title><style>{}</style></head><body><main><h1>{}</h1></main></body></html>",
        STYLE,
        escape_html(message),
    )
}

fn field_keys(spec: &ConfigSpec) -> Vec<&str> {
    spec.fields.iter().map(|field| field.key).collect()
}

fn stored_config_from_map(spec: &ConfigSpec, stored: &BTreeMap<String, String>) -> StoredConfig {
    let mut values = BTreeMap::new();
    for field in &spec.fields {
        if let Some(value) = stored.get(field.key) {
            values.insert(field.key.to_string(), value.clone());
        }
        if matches!(field.kind, FieldKind::Select { .. }) {
            if let Some(other_value) = stored.get(&format!("{}_other", field.key)) {
                if !other_value.is_empty() {
                    values.insert(field.key.to_string(), other_value.clone());
                }
            }
        }
    }

    StoredConfig { values }
}

fn read_existing_config<S>(spec: &ConfigSpec, store: &S) -> Result<StoredConfig>
where
    S: ConfigStore,
{
    Ok(stored_config_from_map(
        spec,
        &store.read(spec.namespace, &field_keys(spec))?,
    ))
}

fn validate_submitted<'a>(
    spec: &ConfigSpec,
    submitted: &BTreeMap<String, String>,
    previous: &StoredConfig,
) -> Result<()> {
    for field in &spec.fields {
        let value = submitted.get(field.key).map(String::as_str).unwrap_or("");
        let has_previous = previous.get(field.key).is_some();

        let required = if matches!(field.kind, FieldKind::Password) {
            field.required && !has_previous
        } else {
            field.required
        };

        if required && value.is_empty() {
            bail!("{} is required", field.label);
        }

        if let FieldKind::Number { min, max } = field.kind {
            if value.is_empty() {
                continue;
            }

            let parsed = value.parse::<i64>().map_err(|_| {
                anyhow!(
                    "{} must be a number between {} and {}",
                    field.label,
                    min,
                    max
                )
            })?;

            if parsed < min || parsed > max {
                bail!(
                    "{} must be a number between {} and {}",
                    field.label,
                    min,
                    max
                );
            }
        }

        if matches!(field.kind, FieldKind::Select { .. }) {
            if value.is_empty() {
                continue;
            }
            if value == "__other__" {
                let other_value = submitted.get(&format!("{}_other", field.key)).map(String::as_str).unwrap_or("");
                if other_value.is_empty() {
                    bail!("{} is required", field.label);
                }
            }
        }
    }

    Ok(())
}

fn stored_field_value<'a>(state: &'a ConfigState, key: &str) -> Option<&'a str> {
    match state {
        ConfigState::Ready(config) => config.get(key),
        ConfigState::Missing => None,
    }
}

fn field_value<'a>(
    field: &FieldSpec,
    state: &'a ConfigState,
    submitted: Option<&'a BTreeMap<String, String>>,
) -> &'a str {
    if matches!(field.kind, FieldKind::Password) {
        return "";
    }

    if let Some(submitted) = submitted {
        if let Some(value) = submitted.get(field.key) {
            if !value.is_empty() && value != "__other__" {
                return value;
            }
            if let Some(other_value) = submitted.get(&format!("{}_other", field.key)) {
                if !other_value.is_empty() {
                    return other_value;
                }
            }
        }
    }

    stored_field_value(state, field.key).unwrap_or("")
}

fn parse_request_form(request: &HttpRequest) -> Result<BTreeMap<String, String>> {
    parse_urlencoded(
        &String::from_utf8(request.body.clone()).context("request body is not valid UTF-8")?,
    )
}

fn parse_urlencoded(input: &str) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();

    for pair in input.split('&') {
        if pair.is_empty() {
            continue;
        }

        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        values.insert(percent_decode(raw_key)?, percent_decode(raw_value)?);
    }

    Ok(values)
}

fn percent_decode(input: &str) -> Result<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                if index + 2 >= bytes.len() {
                    bail!("truncated percent escape");
                }

                let high = decode_hex(bytes[index + 1])?;
                let low = decode_hex(bytes[index + 2])?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    Ok(String::from_utf8(decoded).context("decoded form value is not valid UTF-8")?)
}

fn decode_hex(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => bail!("invalid hex digit"),
    }
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn html_response(body: String) -> HttpResponse {
    HttpResponse {
        status_code: 200,
        content_type: "text/html; charset=utf-8",
        body: body.into_bytes(),
    }
}

fn text_response(status_code: u16, body: &str) -> HttpResponse {
    HttpResponse {
        status_code,
        content_type: "text/plain; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

fn request_method_name(method: &HttpMethod) -> &str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Other(method) => method.as_str(),
    }
}

fn log_request_headers(headers: &BTreeMap<String, String>) {
    for name in [
        "Host",
        "User-Agent",
        "Accept",
        "Accept-Encoding",
        "Accept-Language",
        "Connection",
        "Referer",
        "Cookie",
    ] {
        if let Some(value) = headers.get(name) {
            info!("config portal header: {}={}", name, value);
        }
    }
}

fn default_ap_ip_config() -> IpConfig {
    IpConfig::new("192.168.4.1", "192.168.4.1", "255.255.255.0")
}

fn log_access_point_started(ap_ssid: &str, ip_config: &IpConfig) {
    info!(
        "config portal AP started: ssid={}, ip={}, gateway={}, subnet={}",
        ap_ssid, ip_config.ip, ip_config.gateway, ip_config.netmask
    );
}

const STYLE: &str = "body{font-family:sans-serif;background:#f4f1ea;color:#1d1d1d;margin:0}main{max-width:28rem;margin:0 auto;padding:1.5rem}h1{margin:0 0 1rem;font-size:1.5rem}p{line-height:1.45}form{display:grid;gap:.75rem;margin:1rem 0}label{display:grid;gap:.35rem}input,select,button{font:inherit;padding:.75rem;border-radius:.5rem;border:1px solid #b9b2a7}input:invalid,select:invalid{border:2px solid #8a2f2f}button{background:#1d6b57;color:#fff;border:0}button.danger{background:#8a2f2f}.note{padding:.75rem;border-radius:.5rem;background:#fff7d6}.error{padding:.75rem;border-radius:.5rem;background:#f9d6d6;color:#6c1d1d}.hint{margin:-.4rem 0 0;font-size:.95rem;color:#5b564f}.hidden{display:none}";
