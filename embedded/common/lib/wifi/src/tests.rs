use anyhow::{anyhow, Result};
use std::{collections::VecDeque, time::Duration};

use crate::{
    AccessPointConfig, AccessPointEvent, AccessPointStatus, ClientAuth, ConnectOptions,
    ConnectState, ConnectionInfo, FoundNetwork, IpConfig, Wifi, WifiBackend, WifiCredentials,
};

fn portal_ip_config() -> IpConfig {
    IpConfig::new("192.168.4.1", "192.168.4.1", "255.255.255.0")
}

#[derive(Default)]
struct MockBackend {
    actions: Vec<String>,
    scanned_networks: Vec<FoundNetwork>,
    connected_checks: VecDeque<bool>,
    connection_info: Option<ConnectionInfo>,
    waits: Vec<Duration>,
    started: bool,
    access_point_statuses: VecDeque<AccessPointStatus>,
    access_point_ip_config: Option<IpConfig>,
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
}

impl WifiBackend for MockBackend {
    fn start(&mut self) -> Result<()> {
        self.actions.push("start".to_string());
        self.started = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.actions.push("stop".to_string());
        self.started = false;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<()> {
        self.actions.push("disconnect".to_string());
        Ok(())
    }

    fn is_started(&mut self) -> Result<bool> {
        self.actions.push("is_started".to_string());
        Ok(self.started)
    }

    fn scan_networks(&mut self) -> Result<Vec<FoundNetwork>> {
        self.actions.push("scan".to_string());
        Ok(self.scanned_networks.clone())
    }

    fn configure_client(
        &mut self,
        credentials: &WifiCredentials,
        channel: Option<u8>,
        auth: ClientAuth,
    ) -> Result<()> {
        self.actions.push(format!(
            "configure:{}:{:?}:{:?}",
            credentials.ssid, channel, auth
        ));
        Ok(())
    }

    fn connect(&mut self) -> Result<()> {
        self.actions.push("connect".to_string());
        Ok(())
    }

    fn is_connected(&mut self) -> Result<bool> {
        self.actions.push("is_connected".to_string());
        Ok(self.connected_checks.front().copied().unwrap_or(false))
    }

    fn connection_info(&mut self) -> Result<Option<ConnectionInfo>> {
        self.actions.push("connection_info".to_string());
        if self.connected_checks.front().copied().unwrap_or(false) {
            Ok(self.connection_info.clone())
        } else {
            let _ = self.connected_checks.pop_front();
            Ok(None)
        }
    }

    fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<()> {
        self.actions.push(format!("start_ap:{}", config.ssid));
        self.started = true;
        Ok(())
    }

    fn stop_access_point(&mut self) -> Result<()> {
        self.actions.push("stop_ap".to_string());
        self.started = false;
        Ok(())
    }

    fn access_point_status(&mut self) -> Result<AccessPointStatus> {
        self.actions.push("ap_status".to_string());
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

    fn access_point_ip_config(&mut self) -> Result<IpConfig> {
        self.actions.push("ap_ip_config".to_string());
        self.access_point_ip_config
            .clone()
            .ok_or_else(|| anyhow!("missing AP IP config"))
    }

    fn wait(&mut self, duration: Duration) {
        self.actions.push("wait".to_string());
        self.waits.push(duration);
        let _ = self.connected_checks.pop_front();
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

    let connection = wifi
        .connect_with_options(
            &credentials,
            ConnectOptions {
                timeout: Duration::from_millis(500),
                poll_interval: Duration::from_millis(10),
            },
            |state| states.push(state),
        )
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
            "connect",
            "is_connected",
            "wait",
            "is_connected",
            "connection_info",
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

    wifi.connect(&WifiCredentials::new("guest", ""), |_| {})
        .unwrap();

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

    wifi.connect(&WifiCredentials::new("home", "secret"), |_| {})
        .unwrap();

    assert!(wifi
        .backend()
        .actions
        .contains(&"configure:home:None:Wpa2Personal".to_string()));
}

#[test]
fn connect_requires_non_empty_ssid() {
    let mut wifi = Wifi::new(MockBackend::default());
    let err = wifi
        .connect(&WifiCredentials::new("", "secret"), |_| {})
        .unwrap_err();

    assert_eq!(err.to_string(), anyhow!("Missing WiFi name").to_string());
}

#[test]
fn scan_networks_resets_and_stops_backend() {
    let backend = MockBackend::default().with_scan(vec![FoundNetwork::new("home", Some(3), None)]);
    let mut wifi = Wifi::new(backend);

    let networks = wifi.scan_networks().unwrap();

    assert_eq!(networks, vec![FoundNetwork::new("home", Some(3), None)]);
    assert_eq!(
        wifi.backend().actions,
        vec!["disconnect", "stop", "start", "scan", "stop"]
    );
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

    let ip_config = wifi.start_access_point(&config).unwrap();

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

    wifi.poll_access_point_events(|event| events.push(event))
        .unwrap();
    wifi.poll_access_point_events(|event| events.push(event))
        .unwrap();
    wifi.poll_access_point_events(|event| events.push(event))
        .unwrap();

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
fn start_access_point_requires_non_empty_ssid() {
    let mut wifi = Wifi::new(MockBackend::default());
    let err = wifi
        .start_access_point(&AccessPointConfig::new("", portal_ip_config()))
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        anyhow!("Missing access point name").to_string()
    );
}

#[test]
fn stop_access_point_calls_backend() {
    let mut wifi = Wifi::new(MockBackend::default());

    wifi.stop_access_point().unwrap();

    assert_eq!(wifi.backend().actions, vec!["stop_ap"]);
}
