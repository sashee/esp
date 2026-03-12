use anyhow::{anyhow, bail, Context, Result};
use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_time::{Duration, Instant, Timer};
use embedded_svc::{
    http::{Headers, Method},
    io::Read,
};
use esp_idf_svc::{
    handle::RawHandle,
    http::server::{Configuration as HttpConfiguration, EspHttpServer},
    nvs::{EspNvs, EspNvsPartition, NvsPartitionId},
    sys::{self, ESP_ERR_NVS_NOT_FOUND},
    wifi::{AccessPointConfiguration, AuthMethod, Configuration as WifiConfiguration, EspWifi},
};
use log::{error, info, warn};
use std::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

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
pub struct PortalSpec {
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
pub enum NvsConfigState {
    Missing,
    SchemaMismatch(StoredConfig),
    Ready(StoredConfig),
}

#[derive(Clone, Copy, Debug)]
pub struct PortalTiming {
    pub idle_timeout: Duration,
    pub connected_timeout: Duration,
}

pub async fn enter_config_mode<T>(
    spec: &'static PortalSpec,
    reason: &str,
    wifi: &mut EspWifi<'static>,
    partition: EspNvsPartition<T>,
    timing: PortalTiming,
) -> Result<()>
where
    T: NvsPartitionId + Send + Sync + 'static,
{
    error!("entering config portal: {reason}");

    let ap_ssid = make_ap_ssid(spec.ap_prefix)?;
    start_access_point(wifi, &ap_ssid)?;
    let activity = Arc::new(PortalActivity::default());
    let server = start_http_server(spec, partition, reason.to_string(), activity.clone())?;

    info!("config portal ready on SSID {ap_ssid}");
    let _server = server;

    let started_at = Instant::now();
    let mut client_connected = false;

    loop {
        if !client_connected && softap_has_clients() {
            client_connected = true;
            info!("config portal station connected");
        }

        if activity.reboot_requested.load(Ordering::Relaxed) {
            reboot_now();
        }

        let elapsed = Instant::now() - started_at;
        let limit = if client_connected {
            timing.connected_timeout
        } else {
            timing.idle_timeout
        };

        if elapsed >= limit {
            warn!("config portal timed out after {:?}", elapsed);
            stop_access_point(wifi)?;
            return Ok(());
        }

        if !wifi.is_started().unwrap_or(false) {
            bail!("softap stopped unexpectedly");
        }

        Timer::after(Duration::from_millis(250)).await;
    }
}

pub fn read_nvs<T>(
    spec: &'static PortalSpec,
    partition: EspNvsPartition<T>,
) -> Result<NvsConfigState>
where
    T: NvsPartitionId,
{
    let nvs = match EspNvs::new(partition, spec.namespace, false) {
        Ok(nvs) => nvs,
        Err(err) if err.code() == ESP_ERR_NVS_NOT_FOUND => return Ok(NvsConfigState::Missing),
        Err(err) => return Err(err.into()),
    };
    let expected_schema = schema_signature(spec);
    let stored_schema = read_string(&nvs, SCHEMA_KEY)?;

    let Some(stored_schema) = stored_schema else {
        return Ok(NvsConfigState::Missing);
    };

    if stored_schema != expected_schema {
        return Ok(NvsConfigState::SchemaMismatch(read_existing_values(
            &nvs, spec,
        )?));
    }

    let mut values = BTreeMap::new();
    for field in spec.fields {
        let Some(value) = read_string(&nvs, field.key)? else {
            return Ok(NvsConfigState::Missing);
        };
        values.insert(field.key.to_string(), value);
    }

    Ok(NvsConfigState::Ready(StoredConfig { values }))
}

pub fn clear_nvs<T>(spec: &'static PortalSpec, partition: EspNvsPartition<T>) -> Result<()>
where
    T: NvsPartitionId,
{
    let mut nvs = EspNvs::new(partition, spec.namespace, true)?;
    for field in spec.fields {
        let _ = nvs.remove(field.key)?;
    }
    let _ = nvs.remove(SCHEMA_KEY)?;
    Ok(())
}

pub fn save_to_nvs<T>(
    spec: &'static PortalSpec,
    partition: EspNvsPartition<T>,
    submitted: &BTreeMap<String, String>,
) -> Result<StoredConfig>
where
    T: NvsPartitionId,
{
    let mut nvs = EspNvs::new(partition.clone(), spec.namespace, true)?;
    let previous = read_existing_config(spec, partition)?;
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

        nvs.set_str(field.key, &value)?;
        saved.insert(field.key.to_string(), value);
    }

    nvs.set_str(SCHEMA_KEY, &schema_signature(spec))?;

    Ok(StoredConfig { values: saved })
}

pub fn make_ap_ssid(prefix: &str) -> Result<String> {
    let mut mac = [0_u8; 6];
    esp_idf_svc::sys::esp!(unsafe { sys::esp_efuse_mac_get_default(mac.as_mut_ptr()) })?;

    Ok(format!("{prefix}-{:02X}{:02X}", mac[4], mac[5]))
}

#[derive(Default)]
struct PortalActivity {
    reboot_requested: AtomicBool,
}

fn start_access_point(wifi: &mut EspWifi<'static>, ap_ssid: &str) -> Result<()> {
    let _ = wifi.disconnect();
    let _ = wifi.stop();

    let mut ap = AccessPointConfiguration::default();
    ap.ssid = ap_ssid
        .try_into()
        .map_err(|_| anyhow!("AP SSID is too long"))?;
    ap.channel = 1;
    ap.auth_method = AuthMethod::None;
    ap.max_connections = 1;

    wifi.set_configuration(&WifiConfiguration::AccessPoint(ap))?;
    configure_softap_netif(wifi)?;
    wifi.start()?;
    info!("config portal SoftAP mode started");
    log_heap_status();

    if let Ok(ip_info) = wifi.ap_netif().get_ip_info() {
        info!(
            "config portal AP started: ssid={}, ip={}, gateway={}, subnet={}",
            ap_ssid, ip_info.ip, ip_info.subnet.gateway, ip_info.subnet.mask
        );
    }

    Ok(())
}

fn stop_access_point(wifi: &mut EspWifi<'static>) -> Result<()> {
    let _ = wifi.disconnect();
    wifi.stop()?;
    Ok(())
}

fn start_http_server<T>(
    spec: &'static PortalSpec,
    partition: EspNvsPartition<T>,
    reason: String,
    activity: Arc<PortalActivity>,
) -> Result<EspHttpServer<'static>>
where
    T: NvsPartitionId + Send + Sync + 'static,
{
    let mut server = EspHttpServer::new(&HttpConfiguration::default())?;

    let get_partition = partition.clone();
    let get_reason = reason.clone();
    server.fn_handler::<anyhow::Error, _>("/", Method::Get, move |request| {
        info!("config portal request: GET /");
        log_request_headers(&request);
        let state = read_nvs(spec, get_partition.clone())?;
        let body = render_form(spec, &get_reason, &state, None, None);
        let mut response = request.into_ok_response()?;
        use embedded_svc::io::Write as _;
        response.write_all(body.as_bytes())?;
        Ok(())
    })?;

    let save_partition = partition.clone();
    let save_activity = activity.clone();
    server.fn_handler::<anyhow::Error, _>("/save", Method::Post, move |mut request| {
        info!("config portal request: POST /save");
        log_request_headers(&request);
        let form = read_form_body(&mut request)?;
        if let Err(err) = save_to_nvs(spec, save_partition.clone(), &form) {
            let state = read_nvs(spec, save_partition.clone())?;
            let body = render_form(spec, &reason, &state, Some(&form), Some(&err.to_string()));
            let mut response = request.into_ok_response()?;
            use embedded_svc::io::Write as _;
            response.write_all(body.as_bytes())?;
            return Ok(());
        }

        let body = success_page("Saved configuration. Rebooting...");
        let mut response = request.into_ok_response()?;
        use embedded_svc::io::Write as _;
        response.write_all(body.as_bytes())?;
        save_activity
            .reboot_requested
            .store(true, Ordering::Relaxed);
        Ok(())
    })?;

    let reset_partition = partition;
    let reset_activity = activity;
    server.fn_handler::<anyhow::Error, _>("/reset", Method::Post, move |request| {
        info!("config portal request: POST /reset");
        log_request_headers(&request);
        clear_nvs(spec, reset_partition.clone())?;
        let body = success_page("Reset stored configuration. Rebooting...");
        let mut response = request.into_ok_response()?;
        use embedded_svc::io::Write as _;
        response.write_all(body.as_bytes())?;
        reset_activity
            .reboot_requested
            .store(true, Ordering::Relaxed);
        Ok(())
    })?;

    Ok(server)
}

fn render_form(
    spec: &PortalSpec,
    reason: &str,
    state: &NvsConfigState,
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
        NvsConfigState::Missing => html.push_str("<p class=\"note\">No stored configuration found.</p>"),
        NvsConfigState::SchemaMismatch(_) => {
            html.push_str("<p class=\"note\">Stored configuration does not match the current field schema. Saving will replace it.</p>")
        }
        NvsConfigState::Ready(_) => {}
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

fn schema_signature(spec: &PortalSpec) -> String {
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

fn read_string<T>(nvs: &EspNvs<T>, key: &str) -> Result<Option<String>>
where
    T: NvsPartitionId,
{
    let Some(len) = nvs.str_len(key)? else {
        return Ok(None);
    };
    let mut buf = vec![0_u8; len];
    Ok(nvs.get_str(key, &mut buf)?.map(ToString::to_string))
}

fn read_existing_values<T>(nvs: &EspNvs<T>, spec: &PortalSpec) -> Result<StoredConfig>
where
    T: NvsPartitionId,
{
    let mut values = BTreeMap::new();
    for field in spec.fields {
        if let Some(value) = read_string(nvs, field.key)? {
            values.insert(field.key.to_string(), value);
        }
    }
    Ok(StoredConfig { values })
}

fn read_existing_config<T>(spec: &PortalSpec, partition: EspNvsPartition<T>) -> Result<StoredConfig>
where
    T: NvsPartitionId,
{
    let nvs = match EspNvs::new(partition, spec.namespace, false) {
        Ok(nvs) => nvs,
        Err(err) if err.code() == ESP_ERR_NVS_NOT_FOUND => return Ok(StoredConfig::default()),
        Err(err) => return Err(err.into()),
    };

    read_existing_values(&nvs, spec)
}

fn validate_submitted(
    spec: &PortalSpec,
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

fn stored_field_value<'a>(state: &'a NvsConfigState, key: &str) -> Option<&'a str> {
    match state {
        NvsConfigState::Ready(config) | NvsConfigState::SchemaMismatch(config) => config.get(key),
        NvsConfigState::Missing => None,
    }
}

fn field_value<'a>(
    field: &FieldSpec,
    state: &'a NvsConfigState,
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

fn read_form_body<T>(request: &mut T) -> Result<BTreeMap<String, String>>
where
    T: Read,
    <T as embedded_svc::io::ErrorType>::Error: core::fmt::Debug,
{
    let mut body = Vec::new();
    let mut buf = [0_u8; 256];

    loop {
        let read = request
            .read(&mut buf)
            .map_err(|err| anyhow!("failed reading form body: {err:?}"))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buf[..read]);
    }

    parse_urlencoded(&String::from_utf8(body).context("request body is not valid UTF-8")?)
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

fn softap_has_clients() -> bool {
    let mut list = sys::wifi_sta_list_t::default();
    unsafe { sys::esp_wifi_ap_get_sta_list(&mut list as *mut _) == 0 && list.num > 0 }
}

fn log_request_headers(request: &impl Headers) {
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
        if let Some(value) = request.header(name) {
            info!("config portal header: {}={}", name, value);
        }
    }
}

fn log_heap_status() {
    let caps = sys::MALLOC_CAP_DEFAULT as u32;
    let free = unsafe { sys::heap_caps_get_free_size(caps) };
    let total = unsafe { sys::heap_caps_get_total_size(caps) };
    let largest = unsafe { sys::heap_caps_get_largest_free_block(caps) };

    info!(
        "config portal heap: free={} total={} largest_block={}",
        free, total, largest
    );
}

fn configure_softap_netif(wifi: &mut EspWifi<'_>) -> Result<()> {
    let handle = wifi.ap_netif_mut().handle();
    let mut ip_info = sys::esp_netif_ip_info_t {
        ip: ipv4_addr(192, 168, 4, 1),
        gw: ipv4_addr(192, 168, 4, 1),
        netmask: ipv4_addr(255, 255, 255, 0),
    };

    unsafe {
        sys::esp_netif_dhcps_stop(handle);
    }
    esp_idf_svc::sys::esp!(unsafe { sys::esp_netif_set_ip_info(handle, &mut ip_info as *mut _) })?;
    esp_idf_svc::sys::esp!(unsafe { sys::esp_netif_dhcps_start(handle) })?;

    Ok(())
}

fn ipv4_addr(a: u8, b: u8, c: u8, d: u8) -> sys::esp_ip4_addr_t {
    sys::esp_ip4_addr_t {
        addr: u32::to_be(u32::from_be_bytes([a, b, c, d])),
    }
}

fn reboot_now() -> ! {
    unsafe {
        sys::esp_restart();
    }
}

const STYLE: &str = "body{font-family:sans-serif;background:#f4f1ea;color:#1d1d1d;margin:0}main{max-width:28rem;margin:0 auto;padding:1.5rem}h1{margin:0 0 1rem;font-size:1.5rem}p{line-height:1.45}form{display:grid;gap:.75rem;margin:1rem 0}label{display:grid;gap:.35rem}input,button{font:inherit;padding:.75rem;border-radius:.5rem;border:1px solid #b9b2a7}button{background:#1d6b57;color:#fff;border:0}button.danger{background:#8a2f2f}.note{padding:.75rem;border-radius:.5rem;background:#fff7d6}.error{padding:.75rem;border-radius:.5rem;background:#f9d6d6;color:#6c1d1d}.hint{margin:-.4rem 0 0;font-size:.95rem;color:#5b564f}";
