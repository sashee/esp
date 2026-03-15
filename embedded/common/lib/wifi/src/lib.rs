use anyhow::{bail, Result};
use std::time::{Duration, Instant};

#[cfg(target_os = "espidf")]
pub mod esp_idf;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientAuth {
    Open,
    Wpa2Personal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessPointAuth {
    Open,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WifiCredentials {
    pub ssid: String,
    pub password: String,
}

impl WifiCredentials {
    pub fn new(ssid: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            ssid: ssid.into(),
            password: password.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundNetwork {
    pub ssid: String,
    pub channel: Option<u8>,
    pub signal_strength: Option<i8>,
}

impl FoundNetwork {
    pub fn new(ssid: impl Into<String>, channel: Option<u8>, signal_strength: Option<i8>) -> Self {
        Self {
            ssid: ssid.into(),
            channel,
            signal_strength,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionInfo {
    pub ip: String,
}

impl ConnectionInfo {
    pub fn new(ip: impl Into<String>) -> Self {
        Self { ip: ip.into() }
    }
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
    pub auth: AccessPointAuth,
    pub max_connections: u8,
    pub ip_config: IpConfig,
}

impl AccessPointConfig {
    pub fn new(ssid: impl Into<String>, ip_config: IpConfig) -> Self {
        Self {
            ssid: ssid.into(),
            channel: 1,
            auth: AccessPointAuth::Open,
            max_connections: 1,
            ip_config,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessPointStatus {
    pub is_started: bool,
    pub client_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessPointEvent {
    Started { ip_config: IpConfig },
    ClientCountChanged { client_count: usize },
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectOptions {
    pub timeout: Duration,
    pub poll_interval: Duration,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            poll_interval: Duration::from_millis(250),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectState {
    Starting,
    Scanning,
    ScanComplete {
        networks_found: usize,
    },
    Configuring {
        ssid: String,
        channel: Option<u8>,
        auth: ClientAuth,
    },
    Connecting,
    WaitingForIp,
    Connected {
        ip: String,
    },
}

pub trait WifiBackend {
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn disconnect(&mut self) -> Result<()>;
    fn is_started(&mut self) -> Result<bool>;
    fn scan_networks(&mut self) -> Result<Vec<FoundNetwork>>;
    fn configure_client(
        &mut self,
        credentials: &WifiCredentials,
        channel: Option<u8>,
        auth: ClientAuth,
    ) -> Result<()>;
    fn connect(&mut self) -> Result<()>;
    fn is_connected(&mut self) -> Result<bool>;
    fn connection_info(&mut self) -> Result<Option<ConnectionInfo>>;
    fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<()>;
    fn stop_access_point(&mut self) -> Result<()>;
    fn access_point_status(&mut self) -> Result<AccessPointStatus>;
    fn access_point_ip_config(&mut self) -> Result<IpConfig>;

    fn wait(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

pub struct Wifi<B> {
    backend: B,
    last_access_point_status: Option<AccessPointStatus>,
}

impl<B> Wifi<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            last_access_point_status: None,
        }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn into_inner(self) -> B {
        self.backend
    }
}

impl<B> Wifi<B>
where
    B: WifiBackend,
{
    pub fn scan_networks(&mut self) -> Result<Vec<FoundNetwork>> {
        self.reset()?;
        self.backend.start()?;
        let networks = self.backend.scan_networks();
        let _ = self.backend.stop();
        networks
    }

    pub fn reset(&mut self) -> Result<()> {
        let _ = self.backend.disconnect();
        let _ = self.backend.stop();
        self.last_access_point_status = None;
        Ok(())
    }

    pub fn is_started(&mut self) -> Result<bool> {
        self.backend.is_started()
    }

    pub fn is_connected(&mut self) -> Result<bool> {
        self.backend.is_connected()
    }

    pub fn connection_info(&mut self) -> Result<Option<ConnectionInfo>> {
        self.backend.connection_info()
    }

    pub fn connect<F>(
        &mut self,
        credentials: &WifiCredentials,
        on_state: F,
    ) -> Result<ConnectionInfo>
    where
        F: FnMut(ConnectState),
    {
        self.connect_with_options(credentials, ConnectOptions::default(), on_state)
    }

    pub fn connect_with_options<F>(
        &mut self,
        credentials: &WifiCredentials,
        options: ConnectOptions,
        mut on_state: F,
    ) -> Result<ConnectionInfo>
    where
        F: FnMut(ConnectState),
    {
        validate_credentials(credentials)?;

        on_state(ConnectState::Starting);
        self.reset()?;
        self.backend.start()?;

        on_state(ConnectState::Scanning);
        let networks = self.backend.scan_networks()?;
        on_state(ConnectState::ScanComplete {
            networks_found: networks.len(),
        });

        let channel = select_network_channel(&networks, &credentials.ssid);
        let auth = auth_for_credentials(credentials);
        on_state(ConnectState::Configuring {
            ssid: credentials.ssid.clone(),
            channel,
            auth,
        });
        self.backend.configure_client(credentials, channel, auth)?;

        on_state(ConnectState::Connecting);
        self.backend.connect()?;

        on_state(ConnectState::WaitingForIp);
        let deadline = Instant::now() + options.timeout;

        loop {
            if self.backend.is_connected()? {
                if let Some(connection_info) = self.backend.connection_info()? {
                    on_state(ConnectState::Connected {
                        ip: connection_info.ip.clone(),
                    });
                    return Ok(connection_info);
                }
            }

            if Instant::now() >= deadline {
                bail!("timed out waiting for WiFi connection")
            }

            self.backend.wait(options.poll_interval);
        }
    }

    pub fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<IpConfig> {
        validate_access_point_config(config)?;
        self.reset()?;
        self.backend.start_access_point(config)?;
        let status = self.backend.access_point_status()?;
        self.last_access_point_status = Some(status);
        self.backend.access_point_ip_config()
    }

    pub fn stop_access_point(&mut self) -> Result<()> {
        self.last_access_point_status = None;
        self.backend.stop_access_point()
    }

    pub fn access_point_status(&mut self) -> Result<AccessPointStatus> {
        let status = self.backend.access_point_status()?;
        self.last_access_point_status = Some(status);
        Ok(status)
    }

    pub fn access_point_ip_config(&mut self) -> Result<IpConfig> {
        self.backend.access_point_ip_config()
    }

    pub fn poll_access_point_events<F>(&mut self, mut on_event: F) -> Result<()>
    where
        F: FnMut(AccessPointEvent),
    {
        let status = self.backend.access_point_status()?;
        let previous = self.last_access_point_status.replace(status);

        match previous {
            None => {
                if status.is_started {
                    on_event(AccessPointEvent::Started {
                        ip_config: self.backend.access_point_ip_config()?,
                    });
                    on_event(AccessPointEvent::ClientCountChanged {
                        client_count: status.client_count,
                    });
                }
            }
            Some(previous) => {
                if !previous.is_started && status.is_started {
                    on_event(AccessPointEvent::Started {
                        ip_config: self.backend.access_point_ip_config()?,
                    });
                }

                if previous.client_count != status.client_count {
                    on_event(AccessPointEvent::ClientCountChanged {
                        client_count: status.client_count,
                    });
                }

                if previous.is_started && !status.is_started {
                    on_event(AccessPointEvent::Stopped);
                }
            }
        }

        Ok(())
    }
}

fn validate_credentials(credentials: &WifiCredentials) -> Result<()> {
    if credentials.ssid.is_empty() {
        bail!("Missing WiFi name")
    }

    Ok(())
}

fn validate_access_point_config(config: &AccessPointConfig) -> Result<()> {
    if config.ssid.is_empty() {
        bail!("Missing access point name")
    }

    if config.channel == 0 {
        bail!("Access point channel must be at least 1")
    }

    if config.max_connections == 0 {
        bail!("Access point max_connections must be at least 1")
    }

    Ok(())
}

fn auth_for_credentials(credentials: &WifiCredentials) -> ClientAuth {
    if credentials.password.is_empty() {
        ClientAuth::Open
    } else {
        ClientAuth::Wpa2Personal
    }
}

fn select_network_channel(networks: &[FoundNetwork], ssid: &str) -> Option<u8> {
    networks
        .iter()
        .find(|network| network.ssid == ssid)
        .and_then(|network| network.channel)
}
