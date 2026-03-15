use anyhow::{anyhow, Result};
use std::{
    collections::VecDeque,
    future::Future,
    pin::pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    time::Duration,
};

use crate::{
    AccessPointConfig, AccessPointEvent, AccessPointStatus, ClientAuth, ConnectOptions, ConnectState,
    ConnectionInfo, FoundNetwork, IpConfig, Wifi, WifiBackend, WifiCredentials,
};

fn portal_ip_config() -> IpConfig {
    IpConfig::new("192.168.4.1", "192.168.4.1", "255.255.255.0")
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    fn noop_raw_waker() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            noop_raw_waker()
        }
        fn wake(_: *const ()) {}
        fn wake_by_ref(_: *const ()) {}
        fn drop(_: *const ()) {}

        RawWaker::new(
            core::ptr::null(),
            &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
        )
    }

    let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[derive(Default)]
struct MockBackend {
    actions: Vec<String>,
    scanned_networks: Vec<FoundNetwork>,
    connected_checks: VecDeque<bool>,
    connection_info: Option<ConnectionInfo>,
    started: bool,
    access_point_statuses: VecDeque<AccessPointStatus>,
    access_point_ip_config: Option<IpConfig>,
    start_error: Option<String>,
    disconnect_error: Option<String>,
    stop_error: Option<String>,
    scan_error: Option<String>,
    configure_error: Option<String>,
    connect_error: Option<String>,
    start_access_point_error: Option<String>,
    access_point_status_error: Option<String>,
    access_point_ip_config_error: Option<String>,
}

impl MockBackend {
    fn with_scan(mut self, scanned_networks: Vec<FoundNetwork>) -> Self {
        self.scanned_networks = scanned_networks;
        self
    }

    fn with_connected_checks(mut self, connected_checks: Vec<bool>) -> Self {
        self.connected_checks = connected_checks.into();
        self
    }

    fn with_connection_info(mut self, connection_info: Option<ConnectionInfo>) -> Self {
        self.connection_info = connection_info;
        self
    }

    fn with_access_point_statuses(mut self, statuses: Vec<AccessPointStatus>) -> Self {
        self.access_point_statuses = statuses.into();
        self
    }

    fn with_access_point_ip_config(mut self, config: IpConfig) -> Self {
        self.access_point_ip_config = Some(config);
        self
    }

    fn with_disconnect_error(mut self, error: &str) -> Self {
        self.disconnect_error = Some(error.to_string());
        self
    }

    fn with_start_error(mut self, error: &str) -> Self {
        self.start_error = Some(error.to_string());
        self
    }

    fn with_stop_error(mut self, error: &str) -> Self {
        self.stop_error = Some(error.to_string());
        self
    }

    fn with_scan_error(mut self, error: &str) -> Self {
        self.scan_error = Some(error.to_string());
        self
    }

    fn with_configure_error(mut self, error: &str) -> Self {
        self.configure_error = Some(error.to_string());
        self
    }

    fn with_connect_error(mut self, error: &str) -> Self {
        self.connect_error = Some(error.to_string());
        self
    }

    fn with_access_point_status_error(mut self, error: &str) -> Self {
        self.access_point_status_error = Some(error.to_string());
        self
    }

    fn with_start_access_point_error(mut self, error: &str) -> Self {
        self.start_access_point_error = Some(error.to_string());
        self
    }

    fn with_access_point_ip_config_error(mut self, error: &str) -> Self {
        self.access_point_ip_config_error = Some(error.to_string());
        self
    }
}

impl WifiBackend for MockBackend {
    async fn start(&mut self) -> Result<()> {
        self.actions.push("start".to_string());
        if let Some(error) = &self.start_error {
            return Err(anyhow!(error.clone()));
        }
        self.started = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.actions.push("stop".to_string());
        if let Some(error) = &self.stop_error {
            return Err(anyhow!(error.clone()));
        }
        self.started = false;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.actions.push("disconnect".to_string());
        if let Some(error) = &self.disconnect_error {
            return Err(anyhow!(error.clone()));
        }
        Ok(())
    }

    async fn is_started(&mut self) -> Result<bool> {
        self.actions.push("is_started".to_string());
        Ok(self.started)
    }

    async fn scan_networks(&mut self) -> Result<Vec<FoundNetwork>> {
        self.actions.push("scan".to_string());
        if let Some(error) = &self.scan_error {
            return Err(anyhow!(error.clone()));
        }
        Ok(self.scanned_networks.clone())
    }

    async fn configure_client(
        &mut self,
        credentials: &WifiCredentials,
        channel: Option<u8>,
        auth: ClientAuth,
    ) -> Result<()> {
        self.actions.push(format!(
            "configure:{}:{:?}:{:?}",
            credentials.ssid, channel, auth
        ));
        if let Some(error) = &self.configure_error {
            return Err(anyhow!(error.clone()));
        }
        Ok(())
    }

    async fn connect(&mut self, timeout: Duration) -> Result<ConnectionInfo> {
        self.actions.push(format!("connect:{timeout:?}"));
        if let Some(error) = &self.connect_error {
            return Err(anyhow!(error.clone()));
        }
        while !self.connected_checks.front().copied().unwrap_or(false) {
            let _ = self.connected_checks.pop_front();
        }

        self.connection_info
            .clone()
            .ok_or_else(|| anyhow!("missing connection info"))
    }

    async fn is_connected(&mut self) -> Result<bool> {
        self.actions.push("is_connected".to_string());
        Ok(self.connected_checks.front().copied().unwrap_or(false))
    }

    async fn connection_info(&mut self) -> Result<Option<ConnectionInfo>> {
        self.actions.push("connection_info".to_string());
        if self.connected_checks.front().copied().unwrap_or(false) {
            Ok(self.connection_info.clone())
        } else {
            let _ = self.connected_checks.pop_front();
            Ok(None)
        }
    }

    async fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<()> {
        self.actions.push(format!("start_ap:{}", config.ssid));
        if let Some(error) = &self.start_access_point_error {
            return Err(anyhow!(error.clone()));
        }
        self.started = true;
        Ok(())
    }

    async fn stop_access_point(&mut self) -> Result<()> {
        self.actions.push("stop_ap".to_string());
        self.started = false;
        Ok(())
    }

    async fn access_point_status(&mut self) -> Result<AccessPointStatus> {
        self.actions.push("ap_status".to_string());
        if let Some(error) = &self.access_point_status_error {
            return Err(anyhow!(error.clone()));
        }
        if let Some(status) = self.access_point_statuses.pop_front() {
            self.started = status.is_started;
            Ok(status)
        } else {
            Ok(AccessPointStatus {
                is_started: self.started,
                client_count: 0,
            })
        }
    }

    async fn access_point_ip_config(&mut self) -> Result<IpConfig> {
        self.actions.push("ap_ip_config".to_string());
        if let Some(error) = &self.access_point_ip_config_error {
            return Err(anyhow!(error.clone()));
        }
        self.access_point_ip_config
            .clone()
            .ok_or_else(|| anyhow!("missing AP IP config"))
    }

}

#[test]
fn connect_uses_matching_channel_and_reports_states() {
    let backend = MockBackend::default()
        .with_scan(vec![FoundNetwork::new("home", Some(11), Some(-42))])
        .with_connected_checks(vec![false, true])
        .with_connection_info(Some(ConnectionInfo::new("192.168.1.44")));
    let mut wifi = Wifi::new(backend);
    let credentials = WifiCredentials::new("home", "secret");
    let mut states = Vec::new();

    let connection = block_on(wifi.connect_with_options(
            &credentials,
            ConnectOptions {
                timeout: Duration::from_millis(500),
            },
            |state| states.push(state),
        ))
    .unwrap();

    assert_eq!(connection, ConnectionInfo::new("192.168.1.44"));
    assert_eq!(
        states,
        vec![
            ConnectState::Starting,
            ConnectState::Scanning,
            ConnectState::ScanComplete { networks_found: 1 },
            ConnectState::Configuring {
                ssid: "home".to_string(),
                channel: Some(11),
                auth: ClientAuth::Wpa2Personal,
            },
            ConnectState::Connecting,
            ConnectState::WaitingForIp,
            ConnectState::Connected {
                ip: "192.168.1.44".to_string(),
            },
        ]
    );

    assert_eq!(
        wifi.backend().actions,
        vec![
            "disconnect",
            "stop",
            "start",
            "scan",
            "configure:home:Some(11):Wpa2Personal",
            "connect:500ms",
        ]
    );
}

#[test]
fn connect_uses_open_auth_for_empty_password() {
    let backend = MockBackend::default()
        .with_scan(vec![FoundNetwork::new("guest", Some(6), Some(-55))])
        .with_connected_checks(vec![true])
        .with_connection_info(Some(ConnectionInfo::new("192.168.4.20")));
    let mut wifi = Wifi::new(backend);

    block_on(wifi.connect(&WifiCredentials::new("guest", ""), |_| {})).unwrap();

    assert!(wifi
        .backend()
        .actions
        .contains(&"configure:guest:Some(6):Open".to_string()));
}

#[test]
fn connect_without_matching_network_uses_unknown_channel() {
    let backend = MockBackend::default()
        .with_scan(vec![FoundNetwork::new("other", Some(1), Some(-70))])
        .with_connected_checks(vec![true])
        .with_connection_info(Some(ConnectionInfo::new("10.0.0.5")));
    let mut wifi = Wifi::new(backend);

    block_on(wifi.connect(&WifiCredentials::new("home", "secret"), |_| {})).unwrap();

    assert!(wifi
        .backend()
        .actions
        .contains(&"configure:home:None:Wpa2Personal".to_string()));
}

#[test]
fn connect_requires_non_empty_ssid() {
    let mut wifi = Wifi::new(MockBackend::default());
    let err = block_on(wifi.connect(&WifiCredentials::new("", "secret"), |_| {})).unwrap_err();

    assert_eq!(err.to_string(), anyhow!("Missing WiFi name").to_string());
}

#[test]
fn connect_propagates_start_error() {
    let backend = MockBackend::default().with_start_error("start failed");
    let mut wifi = Wifi::new(backend);
    let mut states = Vec::new();

    let err = block_on(wifi.connect(
        &WifiCredentials::new("home", "secret"),
        |state| states.push(state),
    ))
    .unwrap_err();

    assert_eq!(err.to_string(), "start failed");
    assert_eq!(states, vec![ConnectState::Starting]);
    assert_eq!(wifi.backend().actions, vec!["disconnect", "stop", "start"]);
}

#[test]
fn reset_propagates_disconnect_error() {
    let mut wifi = Wifi::new(MockBackend::default().with_disconnect_error("disconnect failed"));

    let err = block_on(wifi.reset()).unwrap_err();

    assert_eq!(err.to_string(), "disconnect failed");
    assert_eq!(wifi.backend().actions, vec!["disconnect"]);
}

#[test]
fn reset_propagates_stop_error() {
    let mut wifi = Wifi::new(MockBackend::default().with_stop_error("stop failed"));

    let err = block_on(wifi.reset()).unwrap_err();

    assert_eq!(err.to_string(), "stop failed");
    assert_eq!(wifi.backend().actions, vec!["disconnect", "stop"]);
}

#[test]
fn scan_networks_resets_and_stops_backend() {
    let backend = MockBackend::default().with_scan(vec![FoundNetwork::new("home", Some(3), None)]);
    let mut wifi = Wifi::new(backend);

    let networks = block_on(wifi.scan_networks()).unwrap();

    assert_eq!(networks, vec![FoundNetwork::new("home", Some(3), None)]);
    assert_eq!(
        wifi.backend().actions,
        vec!["disconnect", "stop", "start", "scan", "stop"]
    );
}

#[test]
fn connect_propagates_scan_error_and_stops_state_progression() {
    let backend = MockBackend::default().with_scan_error("scan failed");
    let mut wifi = Wifi::new(backend);
    let mut states = Vec::new();

    let err = block_on(wifi.connect(
        &WifiCredentials::new("home", "secret"),
        |state| states.push(state),
    ))
    .unwrap_err();

    assert_eq!(err.to_string(), "scan failed");
    assert_eq!(states, vec![ConnectState::Starting, ConnectState::Scanning]);
    assert_eq!(wifi.backend().actions, vec!["disconnect", "stop", "start", "scan"]);
}

#[test]
fn connect_propagates_configure_error() {
    let backend = MockBackend::default()
        .with_scan(vec![FoundNetwork::new("home", Some(11), Some(-42))])
        .with_configure_error("configure failed");
    let mut wifi = Wifi::new(backend);
    let mut states = Vec::new();

    let err = block_on(wifi.connect(
        &WifiCredentials::new("home", "secret"),
        |state| states.push(state),
    ))
    .unwrap_err();

    assert_eq!(err.to_string(), "configure failed");
    assert_eq!(
        states,
        vec![
            ConnectState::Starting,
            ConnectState::Scanning,
            ConnectState::ScanComplete { networks_found: 1 },
            ConnectState::Configuring {
                ssid: "home".to_string(),
                channel: Some(11),
                auth: ClientAuth::Wpa2Personal,
            },
        ]
    );
    assert_eq!(
        wifi.backend().actions,
        vec![
            "disconnect",
            "stop",
            "start",
            "scan",
            "configure:home:Some(11):Wpa2Personal",
        ]
    );
}

#[test]
fn scan_networks_propagates_start_error() {
    let backend = MockBackend::default().with_start_error("start failed");
    let mut wifi = Wifi::new(backend);

    let err = block_on(wifi.scan_networks()).unwrap_err();

    assert_eq!(err.to_string(), "start failed");
    assert_eq!(wifi.backend().actions, vec!["disconnect", "stop", "start"]);
}

#[test]
fn connect_propagates_backend_connect_error_and_stops_before_connected_state() {
    let backend = MockBackend::default()
        .with_scan(vec![FoundNetwork::new("home", Some(11), Some(-42))])
        .with_connect_error("connect failed");
    let mut wifi = Wifi::new(backend);
    let mut states = Vec::new();

    let err = block_on(wifi.connect_with_options(
        &WifiCredentials::new("home", "secret"),
        ConnectOptions {
            timeout: Duration::from_millis(750),
        },
        |state| states.push(state),
    ))
    .unwrap_err();

    assert_eq!(err.to_string(), "connect failed");
    assert_eq!(
        states,
        vec![
            ConnectState::Starting,
            ConnectState::Scanning,
            ConnectState::ScanComplete { networks_found: 1 },
            ConnectState::Configuring {
                ssid: "home".to_string(),
                channel: Some(11),
                auth: ClientAuth::Wpa2Personal,
            },
            ConnectState::Connecting,
            ConnectState::WaitingForIp,
        ]
    );
    assert_eq!(
        wifi.backend().actions,
        vec![
            "disconnect",
            "stop",
            "start",
            "scan",
            "configure:home:Some(11):Wpa2Personal",
            "connect:750ms",
        ]
    );
}

#[test]
fn connect_passes_timeout_to_backend() {
    let backend = MockBackend::default()
        .with_scan(vec![FoundNetwork::new("home", Some(11), Some(-42))])
        .with_connected_checks(vec![true])
        .with_connection_info(Some(ConnectionInfo::new("192.168.1.44")));
    let mut wifi = Wifi::new(backend);

    block_on(wifi.connect_with_options(
        &WifiCredentials::new("home", "secret"),
        ConnectOptions {
            timeout: Duration::from_secs(3),
        },
        |_| {},
    ))
    .unwrap();

    assert!(wifi.backend().actions.contains(&"connect:3s".to_string()));
}

#[test]
fn start_access_point_returns_ip_config_and_tracks_status() {
    let backend = MockBackend::default()
        .with_access_point_statuses(vec![AccessPointStatus {
            is_started: true,
            client_count: 0,
        }])
        .with_access_point_ip_config(portal_ip_config());
    let mut wifi = Wifi::new(backend);
    let config = AccessPointConfig::new("InfoPanel-1234", portal_ip_config());

    let ip_config = block_on(wifi.start_access_point(&config)).unwrap();

    assert_eq!(ip_config, portal_ip_config());
    assert_eq!(
        wifi.backend().actions,
        vec![
            "disconnect",
            "stop",
            "start_ap:InfoPanel-1234",
            "ap_status",
            "ap_ip_config",
        ]
    );
}

#[test]
fn start_access_point_propagates_backend_start_error() {
    let backend = MockBackend::default()
        .with_access_point_ip_config(portal_ip_config())
        .with_start_access_point_error("ap start failed");
    let mut wifi = Wifi::new(backend);

    let err = block_on(wifi.start_access_point(&AccessPointConfig::new(
        "InfoPanel-1234",
        portal_ip_config(),
    )))
    .unwrap_err();

    assert_eq!(err.to_string(), "ap start failed");
    assert_eq!(
        wifi.backend().actions,
        vec!["disconnect", "stop", "start_ap:InfoPanel-1234"]
    );
}

#[test]
fn start_access_point_propagates_status_error() {
    let backend = MockBackend::default()
        .with_access_point_status_error("ap status failed")
        .with_access_point_ip_config(portal_ip_config());
    let mut wifi = Wifi::new(backend);

    let err = block_on(wifi.start_access_point(&AccessPointConfig::new(
        "InfoPanel-1234",
        portal_ip_config(),
    )))
    .unwrap_err();

    assert_eq!(err.to_string(), "ap status failed");
    assert_eq!(
        wifi.backend().actions,
        vec!["disconnect", "stop", "start_ap:InfoPanel-1234", "ap_status"]
    );
}

#[test]
fn start_access_point_propagates_ip_config_error() {
    let backend = MockBackend::default()
        .with_access_point_statuses(vec![AccessPointStatus {
            is_started: true,
            client_count: 0,
        }])
        .with_access_point_ip_config_error("ap ip failed");
    let mut wifi = Wifi::new(backend);

    let err = block_on(wifi.start_access_point(&AccessPointConfig::new(
        "InfoPanel-1234",
        portal_ip_config(),
    )))
    .unwrap_err();

    assert_eq!(err.to_string(), "ap ip failed");
    assert_eq!(
        wifi.backend().actions,
        vec![
            "disconnect",
            "stop",
            "start_ap:InfoPanel-1234",
            "ap_status",
            "ap_ip_config",
        ]
    );
}

#[test]
fn poll_access_point_events_reports_start_and_client_changes() {
    let backend = MockBackend::default()
        .with_access_point_statuses(vec![
            AccessPointStatus {
                is_started: true,
                client_count: 0,
            },
            AccessPointStatus {
                is_started: true,
                client_count: 1,
            },
            AccessPointStatus {
                is_started: false,
                client_count: 0,
            },
        ])
        .with_access_point_ip_config(portal_ip_config());
    let mut wifi = Wifi::new(backend);
    let mut events = Vec::new();

    block_on(wifi.poll_access_point_events(|event| events.push(event))).unwrap();
    block_on(wifi.poll_access_point_events(|event| events.push(event))).unwrap();
    block_on(wifi.poll_access_point_events(|event| events.push(event))).unwrap();

    assert_eq!(
        events,
        vec![
            AccessPointEvent::Started {
                ip_config: portal_ip_config(),
            },
            AccessPointEvent::ClientCountChanged { client_count: 0 },
            AccessPointEvent::ClientCountChanged { client_count: 1 },
            AccessPointEvent::ClientCountChanged { client_count: 0 },
            AccessPointEvent::Stopped,
        ]
    );
}

#[test]
fn poll_access_point_events_ignores_unchanged_status() {
    let backend = MockBackend::default()
        .with_access_point_statuses(vec![
            AccessPointStatus {
                is_started: true,
                client_count: 0,
            },
            AccessPointStatus {
                is_started: true,
                client_count: 0,
            },
        ])
        .with_access_point_ip_config(portal_ip_config());
    let mut wifi = Wifi::new(backend);
    let mut events = Vec::new();

    block_on(wifi.poll_access_point_events(|event| events.push(event))).unwrap();
    block_on(wifi.poll_access_point_events(|event| events.push(event))).unwrap();

    assert_eq!(
        events,
        vec![
            AccessPointEvent::Started {
                ip_config: portal_ip_config(),
            },
            AccessPointEvent::ClientCountChanged { client_count: 0 },
        ]
    );
}

#[test]
fn poll_access_point_events_propagates_status_error() {
    let backend = MockBackend::default().with_access_point_status_error("ap status failed");
    let mut wifi = Wifi::new(backend);

    let err = block_on(wifi.poll_access_point_events(|_| {})).unwrap_err();

    assert_eq!(err.to_string(), "ap status failed");
    assert_eq!(wifi.backend().actions, vec!["ap_status"]);
}

#[test]
fn poll_access_point_events_propagates_started_ip_error() {
    let backend = MockBackend::default()
        .with_access_point_statuses(vec![AccessPointStatus {
            is_started: true,
            client_count: 0,
        }])
        .with_access_point_ip_config_error("ap ip failed");
    let mut wifi = Wifi::new(backend);

    let err = block_on(wifi.poll_access_point_events(|_| {})).unwrap_err();

    assert_eq!(err.to_string(), "ap ip failed");
    assert_eq!(wifi.backend().actions, vec!["ap_status", "ap_ip_config"]);
}

#[test]
fn access_point_status_does_not_consume_event_baseline() {
    let backend = MockBackend::default()
        .with_access_point_statuses(vec![
            AccessPointStatus {
                is_started: true,
                client_count: 0,
            },
            AccessPointStatus {
                is_started: true,
                client_count: 0,
            },
        ])
        .with_access_point_ip_config(portal_ip_config());
    let mut wifi = Wifi::new(backend);

    let status = block_on(wifi.access_point_status()).unwrap();
    let mut events = Vec::new();
    block_on(wifi.poll_access_point_events(|event| events.push(event))).unwrap();

    assert_eq!(
        status,
        AccessPointStatus {
            is_started: true,
            client_count: 0,
        }
    );
    assert_eq!(
        events,
        vec![
            AccessPointEvent::Started {
                ip_config: portal_ip_config(),
            },
            AccessPointEvent::ClientCountChanged { client_count: 0 },
        ]
    );
}

#[test]
fn start_access_point_requires_non_empty_ssid() {
    let mut wifi = Wifi::new(MockBackend::default());
    let err = block_on(wifi.start_access_point(&AccessPointConfig::new("", portal_ip_config())))
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        anyhow!("Missing access point name").to_string()
    );
}

#[test]
fn start_access_point_rejects_zero_channel() {
    let mut wifi = Wifi::new(MockBackend::default());
    let mut config = AccessPointConfig::new("InfoPanel-1234", portal_ip_config());
    config.channel = 0;

    let err = block_on(wifi.start_access_point(&config)).unwrap_err();

    assert_eq!(err.to_string(), "Access point channel must be at least 1");
}

#[test]
fn start_access_point_rejects_zero_max_connections() {
    let mut wifi = Wifi::new(MockBackend::default());
    let mut config = AccessPointConfig::new("InfoPanel-1234", portal_ip_config());
    config.max_connections = 0;

    let err = block_on(wifi.start_access_point(&config)).unwrap_err();

    assert_eq!(
        err.to_string(),
        "Access point max_connections must be at least 1"
    );
}

#[test]
fn stop_access_point_calls_backend() {
    let mut wifi = Wifi::new(MockBackend::default());

    block_on(wifi.stop_access_point()).unwrap();

    assert_eq!(wifi.backend().actions, vec!["stop_ap"]);
}
