use super::*;
use anyhow::{anyhow, Result};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};
use std::{
    collections::{BTreeMap, VecDeque},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex},
};

static TEST_FIELDS: &[FieldSpec] = &[
    FieldSpec::text("ssid", "Wi-Fi SSID"),
    FieldSpec::password("pw", "Wi-Fi password"),
    FieldSpec::number("brightness", "Brightness", 1, 10),
];

static TEST_SPEC: ConfigSpec = ConfigSpec {
    namespace: "config",
    ap_prefix: "InfoPanel",
    title: "Info Panel Setup",
    fields: TEST_FIELDS,
};

fn block_on<F: Future>(future: F) -> F::Output {
    fn raw_waker() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            raw_waker()
        }
        fn wake(_: *const ()) {}
        fn wake_by_ref(_: *const ()) {}
        fn drop(_: *const ()) {}

        RawWaker::new(
            core::ptr::null(),
            &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
        )
    }

    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(&waker);

    loop {
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn stored(entries: &[(&str, &str)]) -> StoredConfig {
    StoredConfig {
        values: map(entries),
    }
}

fn schema_value() -> String {
    schema_signature(&TEST_SPEC)
}

#[derive(Clone, Default)]
struct MockStore {
    state: Arc<Mutex<MockStoreState>>,
}

#[derive(Default)]
struct MockStoreState {
    values: BTreeMap<String, String>,
    reads: Vec<Vec<String>>,
    writes: Vec<BTreeMap<String, String>>,
    removes: Vec<Vec<String>>,
    read_error: Option<String>,
    write_error: Option<String>,
    remove_error: Option<String>,
}

impl MockStore {
    fn with_values(values: BTreeMap<String, String>) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockStoreState {
                values,
                ..Default::default()
            })),
        }
    }

    fn values(&self) -> BTreeMap<String, String> {
        self.state.lock().unwrap().values.clone()
    }
}

impl ConfigStore for MockStore {
    fn read(&self, keys: &[&str]) -> Result<BTreeMap<String, String>> {
        let mut state = self.state.lock().unwrap();
        state
            .reads
            .push(keys.iter().map(|key| key.to_string()).collect());
        if let Some(message) = state.read_error.clone() {
            return Err(anyhow!(message));
        }

        Ok(keys
            .iter()
            .filter_map(|key| {
                state
                    .values
                    .get(*key)
                    .cloned()
                    .map(|value| ((*key).to_string(), value))
            })
            .collect())
    }

    fn write(&self, values: &BTreeMap<String, String>) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if let Some(message) = state.write_error.clone() {
            return Err(anyhow!(message));
        }
        state.writes.push(values.clone());
        state.values = values.clone();
        Ok(())
    }

    fn remove(&self, keys: &[&str]) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if let Some(message) = state.remove_error.clone() {
            return Err(anyhow!(message));
        }
        let removed: Vec<String> = keys.iter().map(|key| key.to_string()).collect();
        for key in keys {
            state.values.remove(*key);
        }
        state.removes.push(removed);
        Ok(())
    }
}

#[derive(Clone)]
struct MockPlatform {
    state: Arc<Mutex<MockPlatformState>>,
}

#[derive(Default)]
struct MockPlatformState {
    mac: [u8; 6],
    mac_error: Option<String>,
    rebooted: bool,
}

impl MockPlatform {
    fn new(mac: [u8; 6]) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockPlatformState {
                mac,
                ..Default::default()
            })),
        }
    }
}

impl ConfigPlatform for MockPlatform {
    fn mac_address(&self) -> Result<[u8; 6]> {
        let state = self.state.lock().unwrap();
        if let Some(message) = state.mac_error.clone() {
            return Err(anyhow!(message));
        }
        Ok(state.mac)
    }

    fn reboot(&self) -> ! {
        self.state.lock().unwrap().rebooted = true;
        panic!("mock reboot")
    }
}

#[derive(Clone)]
struct MockClock {
    state: Arc<Mutex<MockClockState>>,
}

#[derive(Default)]
struct MockClockState {
    now_values: VecDeque<Instant>,
    sleeps: Vec<Duration>,
}

impl MockClock {
    fn from_ticks(ticks: &[u64]) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockClockState {
                now_values: ticks.iter().copied().map(Instant::from_ticks).collect(),
                sleeps: Vec::new(),
            })),
        }
    }
}

impl ConfigClock for MockClock {
    fn now(&self) -> Instant {
        let mut state = self.state.lock().unwrap();
        if state.now_values.len() > 1 {
            state.now_values.pop_front().unwrap()
        } else {
            *state.now_values.front().unwrap_or(&Instant::from_ticks(0))
        }
    }

    async fn sleep(&self, duration: Duration) {
        self.state.lock().unwrap().sleeps.push(duration);
    }
}

#[derive(Clone, Default)]
struct MockWifi {
    state: Arc<Mutex<MockWifiState>>,
}

struct MockWifiState {
    start_configs: Vec<AccessPointConfig>,
    stop_calls: usize,
    started_result: Result<bool, String>,
    start_result: Result<IpConfig, String>,
    stop_result: Result<(), String>,
    poll_result: Result<(), String>,
    poll_events: VecDeque<Vec<AccessPointEvent>>,
}

impl Default for MockWifiState {
    fn default() -> Self {
        Self {
            start_configs: Vec::new(),
            stop_calls: 0,
            started_result: Ok(true),
            start_result: Ok(default_ap_ip_config()),
            stop_result: Ok(()),
            poll_result: Ok(()),
            poll_events: VecDeque::new(),
        }
    }
}

impl ConfigWifi for MockWifi {
    async fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<IpConfig> {
        let mut state = self.state.lock().unwrap();
        state.start_configs.push(config.clone());
        state
            .start_result
            .clone()
            .map_err(|message| anyhow!(message))
    }

    async fn stop_access_point(&mut self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.stop_calls += 1;
        state
            .stop_result
            .clone()
            .map_err(|message| anyhow!(message))
    }

    async fn is_access_point_started(&mut self) -> Result<bool> {
        self.state
            .lock()
            .unwrap()
            .started_result
            .clone()
            .map_err(|message| anyhow!(message))
    }

    async fn poll_access_point_events<F>(&mut self, mut on_event: F) -> Result<()>
    where
        F: FnMut(AccessPointEvent),
    {
        let mut state = self.state.lock().unwrap();
        let events = state.poll_events.pop_front().unwrap_or_default();
        let result = state.poll_result.clone();
        drop(state);

        for event in events {
            on_event(event);
        }

        result.map_err(|message| anyhow!(message))
    }
}

#[derive(Clone, Default)]
struct MockHttpBackend {
    state: Arc<Mutex<MockHttpBackendState>>,
}

#[derive(Default)]
struct MockHttpBackendState {
    endpoints: Vec<HttpEndpoint>,
    start_error: Option<String>,
    startup_requests: Vec<HttpRequest>,
    startup_responses: Vec<HttpResponse>,
}

impl ConfigHttpBackend for MockHttpBackend {
    type Server = MockServer;

    fn start<H>(self, endpoints: &'static [HttpEndpoint], handler: H) -> Result<Self::Server>
    where
        H: Fn(HttpRequest) -> Result<HttpResponse> + Send + Sync + 'static,
    {
        let mut state = self.state.lock().unwrap();
        if let Some(message) = state.start_error.clone() {
            return Err(anyhow!(message));
        }
        state.endpoints = endpoints.to_vec();
        let requests = std::mem::take(&mut state.startup_requests);
        drop(state);

        let mut responses = Vec::new();
        for request in requests {
            responses.push(handler(request)?);
        }

        self.state.lock().unwrap().startup_responses = responses;
        Ok(MockServer)
    }
}

struct MockServer;

fn request(method: HttpMethod, path: &str, body: &str) -> HttpRequest {
    HttpRequest {
        method,
        path: path.to_string(),
        headers: BTreeMap::new(),
        body: body.as_bytes().to_vec(),
    }
}

fn body_text(response: &HttpResponse) -> String {
    String::from_utf8(response.body.clone()).unwrap()
}

#[test]
fn read_config_returns_missing_without_schema() {
    let store = MockStore::with_values(map(&[("ssid", "home")]));
    assert!(matches!(
        read_config(&TEST_SPEC, &store).unwrap(),
        ConfigState::Missing
    ));
}

#[test]
fn read_config_returns_schema_mismatch() {
    let store = MockStore::with_values(map(&[
        (SCHEMA_KEY, "wrong"),
        ("ssid", "home"),
        ("brightness", "4"),
    ]));

    match read_config(&TEST_SPEC, &store).unwrap() {
        ConfigState::SchemaMismatch(config) => {
            assert_eq!(config.get("ssid"), Some("home"));
            assert_eq!(config.get("brightness"), Some("4"));
        }
        state => panic!("unexpected state: {state:?}"),
    }
}

#[test]
fn read_config_returns_ready_when_complete() {
    let store = MockStore::with_values(map(&[
        (SCHEMA_KEY, &schema_value()),
        ("ssid", "home"),
        ("pw", "secret"),
        ("brightness", "4"),
    ]));

    match read_config(&TEST_SPEC, &store).unwrap() {
        ConfigState::Ready(config) => {
            assert_eq!(config.get("ssid"), Some("home"));
            assert_eq!(config.get("pw"), Some("secret"));
            assert_eq!(config.get("brightness"), Some("4"));
        }
        state => panic!("unexpected state: {state:?}"),
    }
}

#[test]
fn read_config_returns_missing_when_required_field_missing() {
    let store = MockStore::with_values(map(&[
        (SCHEMA_KEY, &schema_value()),
        ("ssid", "home"),
        ("pw", "secret"),
    ]));

    assert!(matches!(
        read_config(&TEST_SPEC, &store).unwrap(),
        ConfigState::Missing
    ));
}

#[test]
fn clear_config_removes_fields_and_schema() {
    let store = MockStore::with_values(map(&[
        (SCHEMA_KEY, &schema_value()),
        ("ssid", "home"),
        ("pw", "secret"),
        ("brightness", "4"),
    ]));

    clear_config(&TEST_SPEC, &store).unwrap();

    assert_eq!(store.values(), BTreeMap::new());
    assert_eq!(
        store.state.lock().unwrap().removes[0],
        vec!["ssid", "pw", "brightness", SCHEMA_KEY]
    );
}

#[test]
fn save_config_writes_values_and_schema() {
    let store = MockStore::default();
    let saved = save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", "secret"), ("brightness", "4")]),
    )
    .unwrap();

    assert_eq!(saved.get("ssid"), Some("home"));
    let written = &store.state.lock().unwrap().writes[0];
    assert_eq!(written.get(SCHEMA_KEY), Some(&schema_value()));
    assert_eq!(written.get("pw"), Some(&"secret".to_string()));
}

#[test]
fn save_config_preserves_stored_password_when_blank() {
    let store = MockStore::with_values(map(&[("pw", "stored")]));
    let saved = save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", ""), ("brightness", "4")]),
    )
    .unwrap();
    assert_eq!(saved.get("pw"), Some("stored"));
}

#[test]
fn save_config_replaces_stored_password_when_present() {
    let store = MockStore::with_values(map(&[("pw", "stored")]));
    let saved = save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", "newpass"), ("brightness", "4")]),
    )
    .unwrap();
    assert_eq!(saved.get("pw"), Some("newpass"));
}

#[test]
fn save_config_rejects_missing_required_text_field() {
    let store = MockStore::default();
    let err = save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", ""), ("pw", "secret"), ("brightness", "4")]),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Wi-Fi SSID is required"));
}

#[test]
fn save_config_rejects_missing_password_without_previous_value() {
    let store = MockStore::default();
    let err = save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", ""), ("brightness", "4")]),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Wi-Fi password is required"));
}

#[test]
fn save_config_accepts_empty_password_with_previous_value() {
    let store = MockStore::with_values(map(&[("pw", "stored")]));
    assert!(save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", ""), ("brightness", "4")])
    )
    .is_ok());
}

#[test]
fn save_config_rejects_non_numeric_value() {
    let store = MockStore::default();
    let err = save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", "secret"), ("brightness", "abc")]),
    )
    .unwrap_err();
    assert!(err
        .to_string()
        .contains("Brightness must be a number between 1 and 10"));
}

#[test]
fn save_config_rejects_out_of_range_value() {
    let store = MockStore::default();
    let err = save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", "secret"), ("brightness", "11")]),
    )
    .unwrap_err();
    assert!(err
        .to_string()
        .contains("Brightness must be a number between 1 and 10"));
}

#[test]
fn save_config_accepts_boundary_numeric_values() {
    let store = MockStore::default();
    assert!(save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", "secret"), ("brightness", "1")])
    )
    .is_ok());
    assert!(save_config(
        &TEST_SPEC,
        &store,
        &map(&[("ssid", "home"), ("pw", "secret"), ("brightness", "10")])
    )
    .is_ok());
}

#[test]
fn ap_ssid_uses_last_two_mac_bytes() {
    assert_eq!(
        ap_ssid("InfoPanel", [0, 1, 2, 3, 0xAB, 0xCD]),
        "InfoPanel-ABCD"
    );
}

#[test]
fn get_root_returns_form_with_stored_values() {
    let store = MockStore::with_values(map(&[
        (SCHEMA_KEY, &schema_value()),
        ("ssid", "home"),
        ("pw", "secret"),
        ("brightness", "4"),
    ]));
    let activity = ConfigActivity::default();

    let response = handle_http_request(
        &TEST_SPEC,
        "configure",
        &store,
        &activity,
        request(HttpMethod::Get, "/", ""),
    )
    .unwrap();
    let body = body_text(&response);
    assert_eq!(response.status_code, 200);
    assert!(body.contains("value=\"home\""));
    assert!(body.contains("value=\"4\""));
}

#[test]
fn get_root_shows_missing_config_note() {
    let store = MockStore::default();
    let body = body_text(
        &handle_http_request(
            &TEST_SPEC,
            "configure",
            &store,
            &ConfigActivity::default(),
            request(HttpMethod::Get, "/", ""),
        )
        .unwrap(),
    );
    assert!(body.contains("No stored configuration found"));
}

#[test]
fn get_root_shows_schema_mismatch_note() {
    let store = MockStore::with_values(map(&[(SCHEMA_KEY, "wrong")]));
    let body = body_text(
        &handle_http_request(
            &TEST_SPEC,
            "configure",
            &store,
            &ConfigActivity::default(),
            request(HttpMethod::Get, "/", ""),
        )
        .unwrap(),
    );
    assert!(body.contains("does not match the current field schema"));
}

#[test]
fn rendered_form_never_prefills_password_field() {
    let state = ConfigState::Ready(stored(&[
        ("ssid", "home"),
        ("pw", "secret"),
        ("brightness", "4"),
    ]));
    let body = render_form(&TEST_SPEC, "configure", &state, None, None);
    assert!(!body.contains("value=\"secret\""));
}

#[test]
fn rendered_form_shows_keep_password_hint_when_password_exists() {
    let state = ConfigState::Ready(stored(&[("pw", "secret")]));
    let body = render_form(&TEST_SPEC, "configure", &state, None, None);
    assert!(body.contains("Leave blank to keep stored password"));
    assert!(body.contains("A password is already stored"));
}

#[test]
fn post_save_returns_success_and_marks_reboot_requested() {
    let store = MockStore::default();
    let activity = ConfigActivity::default();
    let response = handle_http_request(
        &TEST_SPEC,
        "configure",
        &store,
        &activity,
        request(
            HttpMethod::Post,
            "/save",
            "ssid=home&pw=secret&brightness=4",
        ),
    )
    .unwrap();

    assert!(body_text(&response).contains("Saved configuration. Rebooting"));
    assert!(activity.reboot_requested.load(Ordering::Relaxed));
}

#[test]
fn post_save_invalid_form_rerenders_with_error_without_reboot() {
    let store = MockStore::default();
    let activity = ConfigActivity::default();
    let response = handle_http_request(
        &TEST_SPEC,
        "configure",
        &store,
        &activity,
        request(
            HttpMethod::Post,
            "/save",
            "ssid=home&pw=secret&brightness=99",
        ),
    )
    .unwrap();
    let body = body_text(&response);

    assert!(body.contains("Brightness must be a number between 1 and 10"));
    assert!(!activity.reboot_requested.load(Ordering::Relaxed));
}

#[test]
fn post_reset_clears_store_and_marks_reboot_requested() {
    let store = MockStore::with_values(map(&[
        (SCHEMA_KEY, &schema_value()),
        ("ssid", "home"),
        ("pw", "secret"),
        ("brightness", "4"),
    ]));
    let activity = ConfigActivity::default();
    let response = handle_http_request(
        &TEST_SPEC,
        "configure",
        &store,
        &activity,
        request(HttpMethod::Post, "/reset", ""),
    )
    .unwrap();

    assert!(body_text(&response).contains("Reset stored configuration. Rebooting"));
    assert!(activity.reboot_requested.load(Ordering::Relaxed));
    assert!(store.values().is_empty());
}

#[test]
fn unsupported_method_on_known_path_returns_405() {
    let response = handle_http_request(
        &TEST_SPEC,
        "configure",
        &MockStore::default(),
        &ConfigActivity::default(),
        request(HttpMethod::Other("PUT".into()), "/", ""),
    )
    .unwrap();
    assert_eq!(response.status_code, 405);
}

#[test]
fn unknown_path_returns_404() {
    let response = handle_http_request(
        &TEST_SPEC,
        "configure",
        &MockStore::default(),
        &ConfigActivity::default(),
        request(HttpMethod::Get, "/missing", ""),
    )
    .unwrap();
    assert_eq!(response.status_code, 404);
}

#[test]
fn parse_request_form_handles_urlencoded_values() {
    let form = parse_request_form(&request(
        HttpMethod::Post,
        "/save",
        "ssid=hello+world&pw=a%2Bb&brightness=",
    ))
    .unwrap();
    assert_eq!(form.get("ssid"), Some(&"hello world".to_string()));
    assert_eq!(form.get("pw"), Some(&"a+b".to_string()));
    assert_eq!(form.get("brightness"), Some(&String::new()));
}

#[test]
fn malformed_percent_escape_returns_error() {
    let err = parse_urlencoded("ssid=bad%2").unwrap_err();
    assert!(err.to_string().contains("truncated percent escape"));
}

#[test]
fn parse_urlencoded_handles_multiple_keys() {
    let form = parse_urlencoded("a=1&b=two&c=three").unwrap();
    assert_eq!(form.get("a"), Some(&"1".to_string()));
    assert_eq!(form.get("b"), Some(&"two".to_string()));
    assert_eq!(form.get("c"), Some(&"three".to_string()));
}

#[test]
fn percent_decode_handles_plus_and_percent20() {
    assert_eq!(
        percent_decode("hello+there%20friend").unwrap(),
        "hello there friend"
    );
}

#[test]
fn escape_html_escapes_special_characters() {
    assert_eq!(escape_html("<&>\"'"), "&lt;&amp;&gt;&quot;&#39;");
}

#[test]
fn field_value_prefers_submitted_value_for_non_password_fields() {
    let state = ConfigState::Ready(stored(&[("ssid", "old")]));
    let submitted = map(&[("ssid", "new")]);
    assert_eq!(
        field_value(&TEST_FIELDS[0], &state, Some(&submitted)),
        "new"
    );
}

#[test]
fn field_value_never_returns_stored_password() {
    let state = ConfigState::Ready(stored(&[("pw", "secret")]));
    assert_eq!(field_value(&TEST_FIELDS[1], &state, None), "");
}

#[test]
fn stored_field_value_handles_all_config_states() {
    assert_eq!(
        stored_field_value(&ConfigState::Ready(stored(&[("ssid", "home")])), "ssid"),
        Some("home")
    );
    assert_eq!(
        stored_field_value(
            &ConfigState::SchemaMismatch(stored(&[("ssid", "home")])),
            "ssid"
        ),
        Some("home")
    );
    assert_eq!(stored_field_value(&ConfigState::Missing, "ssid"), None);
}

#[test]
fn enter_config_mode_starts_ap_with_expected_settings() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            stop_result: Ok(()),
            started_result: Ok(false),
            ..Default::default()
        })),
    };
    let http = MockHttpBackend::default();
    let platform = MockPlatform::new([0, 1, 2, 3, 0xAA, 0xBB]);
    let clock = MockClock::from_ticks(&[0, 1000]);

    let _ = block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        http,
        platform,
        clock,
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ));

    let config = &wifi.state.lock().unwrap().start_configs[0];
    assert_eq!(config.ssid, "InfoPanel-AABB");
    assert_eq!(config.channel, 1);
    assert_eq!(config.max_connections, 1);
    assert_eq!(config.ip_config, default_ap_ip_config());
}

#[test]
fn enter_config_mode_times_out_without_client() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            stop_result: Ok(()),
            started_result: Ok(true),
            ..Default::default()
        })),
    };
    let clock = MockClock::from_ticks(&[0, 0, 600]);

    block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        MockHttpBackend::default(),
        MockPlatform::new([0, 0, 0, 0, 0, 1]),
        clock.clone(),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap();

    assert_eq!(wifi.state.lock().unwrap().stop_calls, 1);
    assert_eq!(
        clock.state.lock().unwrap().sleeps,
        vec![Duration::from_millis(250)]
    );
}

#[test]
fn enter_config_mode_uses_connected_timeout_after_client_connects() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            stop_result: Ok(()),
            started_result: Ok(true),
            poll_events: VecDeque::from([
                vec![AccessPointEvent::ClientCountChanged { client_count: 1 }],
                vec![],
            ]),
            ..Default::default()
        })),
    };
    let clock = MockClock::from_ticks(&[0, 600, 1100]);

    block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        MockHttpBackend::default(),
        MockPlatform::new([0, 0, 0, 0, 0, 1]),
        clock.clone(),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap();

    assert_eq!(wifi.state.lock().unwrap().stop_calls, 1);
    assert_eq!(clock.state.lock().unwrap().sleeps.len(), 1);
}

#[test]
fn enter_config_mode_reboots_after_http_request_marks_activity() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            stop_result: Ok(()),
            started_result: Ok(true),
            ..Default::default()
        })),
    };
    let http = MockHttpBackend {
        state: Arc::new(Mutex::new(MockHttpBackendState {
            startup_requests: vec![request(HttpMethod::Post, "/reset", "")],
            ..Default::default()
        })),
    };
    let platform = MockPlatform::new([0, 0, 0, 0, 0, 1]);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = block_on(enter_config_mode(
            &TEST_SPEC,
            "configure",
            &mut wifi,
            MockStore::default(),
            http.clone(),
            platform.clone(),
            MockClock::from_ticks(&[0]),
            ConfigTiming {
                idle_timeout: Duration::from_ticks(500),
                connected_timeout: Duration::from_ticks(1000),
            },
        ));
    }));

    assert!(result.is_err());
    assert!(platform.state.lock().unwrap().rebooted);
    assert!(body_text(&http.state.lock().unwrap().startup_responses[0])
        .contains("Reset stored configuration. Rebooting"));
}

#[test]
fn enter_config_mode_errors_when_ap_stops_unexpectedly() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            started_result: Ok(false),
            ..Default::default()
        })),
    };

    let err = block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        MockHttpBackend::default(),
        MockPlatform::new([0, 0, 0, 0, 0, 1]),
        MockClock::from_ticks(&[0, 0]),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap_err();

    assert!(err.to_string().contains("softap stopped unexpectedly"));
}

#[test]
fn enter_config_mode_propagates_ap_start_failure() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Err("start failed".into()),
            ..Default::default()
        })),
    };

    let err = block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        MockHttpBackend::default(),
        MockPlatform::new([0, 0, 0, 0, 0, 1]),
        MockClock::from_ticks(&[0]),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap_err();

    assert!(err.to_string().contains("start failed"));
}

#[test]
fn enter_config_mode_propagates_ap_stop_failure() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            stop_result: Err("stop failed".into()),
            started_result: Ok(true),
            ..Default::default()
        })),
    };

    let err = block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        MockHttpBackend::default(),
        MockPlatform::new([0, 0, 0, 0, 0, 1]),
        MockClock::from_ticks(&[0, 0, 600]),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap_err();

    assert!(err.to_string().contains("stop failed"));
}

#[test]
fn enter_config_mode_propagates_wifi_poll_failure() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            poll_result: Err("poll failed".into()),
            ..Default::default()
        })),
    };

    let err = block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        MockHttpBackend::default(),
        MockPlatform::new([0, 0, 0, 0, 0, 1]),
        MockClock::from_ticks(&[0]),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap_err();

    assert!(err.to_string().contains("poll failed"));
}

#[test]
fn enter_config_mode_propagates_started_check_failure() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            started_result: Err("started failed".into()),
            ..Default::default()
        })),
    };

    let err = block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        MockHttpBackend::default(),
        MockPlatform::new([0, 0, 0, 0, 0, 1]),
        MockClock::from_ticks(&[0, 0]),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap_err();

    assert!(err.to_string().contains("started failed"));
}

#[test]
fn enter_config_mode_propagates_http_backend_start_failure() {
    let mut wifi = MockWifi {
        state: Arc::new(Mutex::new(MockWifiState {
            start_result: Ok(default_ap_ip_config()),
            ..Default::default()
        })),
    };
    let http = MockHttpBackend {
        state: Arc::new(Mutex::new(MockHttpBackendState {
            start_error: Some("http failed".into()),
            ..Default::default()
        })),
    };

    let err = block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        http,
        MockPlatform::new([0, 0, 0, 0, 0, 1]),
        MockClock::from_ticks(&[0]),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap_err();

    assert!(err.to_string().contains("http failed"));
}

#[test]
fn get_request_propagates_store_read_failure() {
    let store = MockStore::default();
    store.state.lock().unwrap().read_error = Some("read failed".into());
    let err = handle_http_request(
        &TEST_SPEC,
        "configure",
        &store,
        &ConfigActivity::default(),
        request(HttpMethod::Get, "/", ""),
    )
    .unwrap_err();
    assert!(err.to_string().contains("read failed"));
}

#[test]
fn post_reset_propagates_store_clear_failure() {
    let store = MockStore::default();
    store.state.lock().unwrap().remove_error = Some("clear failed".into());
    let err = handle_http_request(
        &TEST_SPEC,
        "configure",
        &store,
        &ConfigActivity::default(),
        request(HttpMethod::Post, "/reset", ""),
    )
    .unwrap_err();
    assert!(err.to_string().contains("clear failed"));
}

#[test]
fn post_save_re_renders_when_store_write_fails() {
    let store = MockStore::default();
    store.state.lock().unwrap().write_error = Some("write failed".into());
    let response = handle_http_request(
        &TEST_SPEC,
        "configure",
        &store,
        &ConfigActivity::default(),
        request(
            HttpMethod::Post,
            "/save",
            "ssid=home&pw=secret&brightness=4",
        ),
    )
    .unwrap();

    assert!(body_text(&response).contains("write failed"));
}

#[test]
fn enter_config_mode_propagates_mac_address_failure() {
    let mut wifi = MockWifi::default();
    let platform = MockPlatform::new([0, 0, 0, 0, 0, 1]);
    platform.state.lock().unwrap().mac_error = Some("mac failed".into());

    let err = block_on(enter_config_mode(
        &TEST_SPEC,
        "configure",
        &mut wifi,
        MockStore::default(),
        MockHttpBackend::default(),
        platform,
        MockClock::from_ticks(&[0]),
        ConfigTiming {
            idle_timeout: Duration::from_ticks(500),
            connected_timeout: Duration::from_ticks(1000),
        },
    ))
    .unwrap_err();

    assert!(err.to_string().contains("mac failed"));
    assert!(wifi.state.lock().unwrap().start_configs.is_empty());
}
