use anyhow::{bail, Result};
use std::time::Duration;

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

#[allow(async_fn_in_trait)]
pub trait AccessPointClientConnectedSubscription {
    async fn next(&mut self) -> Result<()>;
}

#[allow(async_fn_in_trait)]
pub trait AccessPointStoppedSubscription {
    async fn next(&mut self) -> Result<()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectOptions {
    pub timeout: Duration,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
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

#[allow(async_fn_in_trait)]
pub trait WifiBackend {
    type AccessPointClientConnectedSubscription: AccessPointClientConnectedSubscription;
    type AccessPointStoppedSubscription: AccessPointStoppedSubscription;

    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn scan_networks(&mut self) -> Result<Vec<FoundNetwork>>;
    async fn configure_client(
        &mut self,
        credentials: &WifiCredentials,
        channel: Option<u8>,
        auth: ClientAuth,
    ) -> Result<()>;
    async fn connect(&mut self, timeout: Duration) -> Result<ConnectionInfo>;
    async fn is_connected(&mut self) -> Result<bool>;
    async fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<IpConfig>;
    async fn stop_access_point(&mut self) -> Result<()>;
    fn subscribe_access_point_client_connected(
        &self,
    ) -> Result<Self::AccessPointClientConnectedSubscription>;
    fn subscribe_access_point_stopped(&self) -> Result<Self::AccessPointStoppedSubscription>;
}

pub struct Wifi<B> {
    backend: B,
}

impl<B> Wifi<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B> Wifi<B>
where
    B: WifiBackend,
{
    pub async fn scan_networks(&mut self) -> Result<Vec<FoundNetwork>> {
        self.reset().await?;
        self.backend.start().await?;
        let networks = self.backend.scan_networks().await;
        let _ = self.backend.stop().await;
        networks
    }

    pub async fn reset(&mut self) -> Result<()> {
        self.backend.disconnect().await?;
        self.backend.stop().await?;
        Ok(())
    }

    pub async fn is_connected(&mut self) -> Result<bool> {
        self.backend.is_connected().await
    }

    pub async fn connect<F>(
        &mut self,
        credentials: &WifiCredentials,
        on_state: F,
    ) -> Result<ConnectionInfo>
    where
        F: FnMut(ConnectState),
    {
        self.connect_with_options(credentials, ConnectOptions::default(), on_state)
            .await
    }

    pub async fn connect_with_options<F>(
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
        self.reset().await?;
        self.backend.start().await?;

        on_state(ConnectState::Scanning);
        let networks = self.backend.scan_networks().await?;
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
        self.backend
            .configure_client(credentials, channel, auth)
            .await?;

        on_state(ConnectState::Connecting);
        on_state(ConnectState::WaitingForIp);
        let connection_info = self.backend.connect(options.timeout).await?;

        on_state(ConnectState::Connected {
            ip: connection_info.ip.clone(),
        });

        Ok(connection_info)
    }

    pub async fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<IpConfig> {
        validate_access_point_config(config)?;
        self.reset().await?;
        self.backend.start_access_point(config).await
    }

    pub async fn stop_access_point(&mut self) -> Result<()> {
        self.backend.stop_access_point().await
    }

    pub fn subscribe_access_point_client_connected(
        &self,
    ) -> Result<B::AccessPointClientConnectedSubscription> {
        self.backend.subscribe_access_point_client_connected()
    }

    pub fn subscribe_access_point_stopped(&self) -> Result<B::AccessPointStoppedSubscription> {
        self.backend.subscribe_access_point_stopped()
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
