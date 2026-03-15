#[cfg(target_os = "espidf")]
pub mod esp_idf;

use anyhow::{anyhow, bail, Context, Result};
use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_time::{Duration, Instant};
use log::{error, info, warn};
use std::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use wifi::{AccessPointConfig, AccessPointEvent, IpConfig, Wifi as WifiController, WifiBackend};

const SCHEMA_KEY: &str = "_schema";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Text,
    Password,
    Number { min: i64, max: i64 },
}

#[derive(Clone, Copy, Debug)]
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
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigSpec {
    pub namespace: &'static str,
    pub ap_prefix: &'static str,
    pub title: &'static str,
    pub fields: &'static [FieldSpec],
}

#[derive(Clone, Debug, Default)]
pub struct StoredConfig {
    values: BTreeMap<String, String>,
}

impl StoredConfig {
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
    SchemaMismatch(StoredConfig),
    Ready(StoredConfig),
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigTiming {
    pub idle_timeout: Duration,
    pub connected_timeout: Duration,
}

pub trait ConfigStore {
    fn read(&self, keys: &[&str]) -> Result<BTreeMap<String, String>>;

    fn write(&self, values: &BTreeMap<String, String>) -> Result<()>;

    fn remove(&self, keys: &[&str]) -> Result<()>;
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

    fn start<H>(self, endpoints: &'static [HttpEndpoint], handler: H) -> Result<Self::Server>
    where
        H: Fn(HttpRequest) -> Result<HttpResponse> + Send + Sync + 'static;
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

pub async fn enter_config_mode<B, S, H, P, C>(
    spec: &'static ConfigSpec,
    reason: &str,
    wifi: &mut WifiController<B>,
    store: S,
    http: H,
    platform: P,
    clock: C,
    timing: ConfigTiming,
) -> Result<()>
where
    B: WifiBackend,
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
        handle_http_request(spec, &reason, &store, &http_activity, request)
    })?;

    info!("config portal ready on SSID {ap_ssid}");
    let _server = server;

    let started_at = clock.now();
    let mut client_connected = false;

    loop {
        wifi.poll_access_point_events(|event| match event {
            AccessPointEvent::Started { ip_config } => {
                info!(
                    "config portal AP started: ssid={}, ip={}, gateway={}, subnet={}",
                    ap_ssid, ip_config.ip, ip_config.gateway, ip_config.netmask
                );
            }
            AccessPointEvent::ClientCountChanged { client_count } => {
                if !client_connected && client_count > 0 {
                    client_connected = true;
                    info!("config portal station connected");
                }
            }
            AccessPointEvent::Stopped => {}
        })
        .await?;

        if activity.reboot_requested.load(Ordering::Relaxed) {
            platform.reboot();
        }

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

        if !wifi.is_started().await? {
            bail!("softap stopped unexpectedly");
        }

        clock.sleep(Duration::from_millis(250)).await;
    }
}

pub fn read_config<S>(spec: &'static ConfigSpec, store: &S) -> Result<ConfigState>
where
    S: ConfigStore,
{
    let stored = store.read(&spec_keys(spec))?;
    let Some(stored_schema) = stored.get(SCHEMA_KEY) else {
        return Ok(ConfigState::Missing);
    };

    if stored_schema != &schema_signature(spec) {
        return Ok(ConfigState::SchemaMismatch(stored_config_from_map(
            spec, &stored,
        )));
    }

    let mut values = BTreeMap::new();
    for field in spec.fields {
        let Some(value) = stored.get(field.key) else {
            return Ok(ConfigState::Missing);
        };
        values.insert(field.key.to_string(), value.clone());
    }

    Ok(ConfigState::Ready(StoredConfig { values }))
}

pub fn clear_config<S>(spec: &'static ConfigSpec, store: &S) -> Result<()>
where
    S: ConfigStore,
{
    store.remove(&spec_keys(spec))
}

pub fn save_config<S>(
    spec: &'static ConfigSpec,
    store: &S,
    submitted: &BTreeMap<String, String>,
) -> Result<StoredConfig>
where
    S: ConfigStore,
{
    let previous = read_existing_config(spec, store)?;
    validate_submitted(spec, submitted, &previous)?;

    let mut saved = BTreeMap::new();
    for field in spec.fields {
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
        };
        saved.insert(field.key.to_string(), value);
    }

    let mut persisted = saved.clone();
    persisted.insert(SCHEMA_KEY.to_string(), schema_signature(spec));
    store.write(&persisted)?;

    Ok(StoredConfig { values: saved })
}

pub fn ap_ssid(prefix: &str, mac: [u8; 6]) -> String {
    format!("{prefix}-{:02X}{:02X}", mac[4], mac[5])
}

#[derive(Default)]
struct ConfigActivity {
    reboot_requested: AtomicBool,
}

async fn start_access_point<B>(wifi: &mut WifiController<B>, ap_ssid: &str) -> Result<()>
where
    B: WifiBackend,
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

async fn stop_access_point<B>(wifi: &mut WifiController<B>) -> Result<()>
where
    B: WifiBackend,
{
    wifi.stop_access_point().await
}

fn handle_http_request<S>(
    spec: &'static ConfigSpec,
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
            Ok(html_response(render_form(spec, reason, &state, None, None)))
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
                )));
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

fn render_form(
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
        ConfigState::SchemaMismatch(_) => {
            html.push_str("<p class=\"note\">Stored configuration does not match the current field schema. Saving will replace it.</p>")
        }
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
    for field in spec.fields {
        let value = field_value(field, state, submitted);
        let has_stored_value = stored_field_value(state, field.key).is_some();
        let input_type = match field.kind {
            FieldKind::Text => "text",
            FieldKind::Password => "password",
            FieldKind::Number { .. } => "number",
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
        if let FieldKind::Number { min, max } = field.kind {
            let _ = write!(extra_attrs, " min=\"{}\" max=\"{}\" step=\"1\"", min, max);
        }

        let _ = write!(
            html,
            "<label><span>{}</span><input type=\"{}\" name=\"{}\" value=\"{}\" autocomplete=\"off\"{}{}></label>",
            escape_html(field.label),
            input_type,
            escape_html(field.key),
            escape_html(value),
            required_attr,
            extra_attrs,
        );

        if matches!(field.kind, FieldKind::Password) && has_stored_value {
            html.push_str(
                "<p class=\"hint\">A password is already stored; leave blank to keep it.</p>",
            );
        }
    }
    html.push_str("<button type=\"submit\">Save and reboot</button></form>");
    html.push_str("<form method=\"post\" action=\"/reset\"><button class=\"danger\" type=\"submit\">Reset stored config</button></form>");
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

fn schema_signature(spec: &ConfigSpec) -> String {
    let mut schema = String::new();
    schema.push_str(spec.namespace);
    schema.push('|');
    schema.push_str(spec.title);

    for field in spec.fields {
        schema.push('|');
        schema.push_str(field.key);
        schema.push(':');
        match field.kind {
            FieldKind::Text => schema.push_str("text"),
            FieldKind::Password => schema.push_str("password"),
            FieldKind::Number { min, max } => {
                let _ = write!(schema, "number({},{})", min, max);
            }
        }
        schema.push(':');
        schema.push_str(if field.required {
            "required"
        } else {
            "optional"
        });
    }

    schema
}

fn spec_keys(spec: &ConfigSpec) -> Vec<&str> {
    let mut keys = field_keys(spec);
    keys.push(SCHEMA_KEY);
    keys
}

fn field_keys(spec: &ConfigSpec) -> Vec<&str> {
    spec.fields.iter().map(|field| field.key).collect()
}

fn stored_config_from_map(spec: &ConfigSpec, stored: &BTreeMap<String, String>) -> StoredConfig {
    let mut values = BTreeMap::new();
    for field in spec.fields {
        if let Some(value) = stored.get(field.key) {
            values.insert(field.key.to_string(), value.clone());
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
        &store.read(&field_keys(spec))?,
    ))
}

fn validate_submitted(
    spec: &ConfigSpec,
    submitted: &BTreeMap<String, String>,
    previous: &StoredConfig,
) -> Result<()> {
    for field in spec.fields {
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
    }

    Ok(())
}

fn stored_field_value<'a>(state: &'a ConfigState, key: &str) -> Option<&'a str> {
    match state {
        ConfigState::Ready(config) | ConfigState::SchemaMismatch(config) => config.get(key),
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
            return value;
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

const STYLE: &str = "body{font-family:sans-serif;background:#f4f1ea;color:#1d1d1d;margin:0}main{max-width:28rem;margin:0 auto;padding:1.5rem}h1{margin:0 0 1rem;font-size:1.5rem}p{line-height:1.45}form{display:grid;gap:.75rem;margin:1rem 0}label{display:grid;gap:.35rem}input,button{font:inherit;padding:.75rem;border-radius:.5rem;border:1px solid #b9b2a7}button{background:#1d6b57;color:#fff;border:0}button.danger{background:#8a2f2f}.note{padding:.75rem;border-radius:.5rem;background:#fff7d6}.error{padding:.75rem;border-radius:.5rem;background:#f9d6d6;color:#6c1d1d}.hint{margin:-.4rem 0 0;font-size:.95rem;color:#5b564f}";
