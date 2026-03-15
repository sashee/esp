use anyhow::Result;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::WifiModemPeripheral,
    handle::RawHandle,
    nvs::EspDefaultNvsPartition,
    sys,
    timer::EspTaskTimerService,
    wifi::{
        AccessPointConfiguration, AsyncWifi, AuthMethod, ClientConfiguration, Configuration,
        EspWifi,
    },
};
use std::time::Duration;
use std::vec::Vec;

use crate::{
    AccessPointAuth, AccessPointConfig, AccessPointStatus, ClientAuth, ConnectionInfo,
    FoundNetwork, IpConfig, WifiBackend, WifiCredentials,
};

fn ignore_wifi_state_error(err: esp_idf_svc::sys::EspError) -> Result<()> {
    if err.code() == sys::ESP_ERR_WIFI_NOT_STARTED || err.code() == sys::ESP_ERR_WIFI_NOT_CONNECT {
        Ok(())
    } else {
        Err(err.into())
    }
}

pub struct EspWifiBackend<'d> {
    wifi: AsyncWifi<EspWifi<'d>>,
}

impl<'d> EspWifiBackend<'d> {
    pub fn new(wifi: EspWifi<'d>, sysloop: EspSystemEventLoop) -> Result<Self> {
        let timer_service = EspTaskTimerService::new()?;
        let wifi = AsyncWifi::wrap(wifi, sysloop, timer_service)?;
        Ok(Self { wifi })
    }

    pub fn new_with_default_nvs(
        modem: impl WifiModemPeripheral + 'static,
        sysloop: EspSystemEventLoop,
        nvs: Option<EspDefaultNvsPartition>,
    ) -> Result<Self> {
        let wifi = EspWifi::new(modem, sysloop.clone(), nvs)?;
        Self::new(wifi, sysloop)
    }
}

impl<'d> WifiBackend for EspWifiBackend<'d> {
    async fn start(&mut self) -> Result<()> {
        self.wifi
            .set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
        self.wifi.start().await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        match self.wifi.stop().await {
            Ok(()) => Ok(()),
            Err(err) => ignore_wifi_state_error(err),
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        match self.wifi.disconnect().await {
            Ok(()) => Ok(()),
            Err(err) => ignore_wifi_state_error(err),
        }
    }

    async fn is_started(&mut self) -> Result<bool> {
        Ok(self.wifi.is_started().unwrap_or(false))
    }

    async fn scan_networks(&mut self) -> Result<Vec<FoundNetwork>> {
        let access_points = self.wifi.scan().await?;

        Ok(access_points
            .into_iter()
            .map(|access_point| FoundNetwork {
                ssid: access_point.ssid.to_string(),
                channel: Some(access_point.channel),
                signal_strength: Some(access_point.signal_strength),
            })
            .collect())
    }

    async fn configure_client(
        &mut self,
        credentials: &WifiCredentials,
        channel: Option<u8>,
        auth: ClientAuth,
    ) -> Result<()> {
        let mut client = ClientConfiguration::default();
        client.ssid = credentials
            .ssid
            .as_str()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Could not parse the given SSID into WiFi config"))?;
        client.password =
            credentials.password.as_str().try_into().map_err(|_| {
                anyhow::anyhow!("Could not parse the given password into WiFi config")
            })?;
        client.channel = channel;
        client.auth_method = match auth {
            ClientAuth::Open => AuthMethod::None,
            ClientAuth::Wpa2Personal => AuthMethod::WPA2Personal,
        };

        self.wifi
            .set_configuration(&Configuration::Client(client))?;
        Ok(())
    }

    async fn connect(&mut self, timeout: Duration) -> Result<ConnectionInfo> {
        self.wifi.wifi_mut().connect()?;
        self.wifi
            .wifi_wait(|this| this.wifi().is_connected().map(|s| !s), Some(timeout))
            .await?;
        self.wifi
            .ip_wait_while(|this| this.wifi().is_up().map(|s| !s), Some(timeout))
            .await?;

        self.connection_info()
            .await?
            .ok_or_else(|| anyhow::anyhow!("WiFi connected without IP configuration"))
    }

    async fn is_connected(&mut self) -> Result<bool> {
        Ok(self.wifi.is_connected().unwrap_or(false))
    }

    async fn connection_info(&mut self) -> Result<Option<ConnectionInfo>> {
        let ip_info = match self.wifi.wifi().sta_netif().get_ip_info() {
            Ok(ip_info) => ip_info,
            Err(_) => return Ok(None),
        };

        let ip = ip_info.ip.to_string();
        if ip == "0.0.0.0" {
            return Ok(None);
        }

        Ok(Some(ConnectionInfo { ip }))
    }

    async fn start_access_point(&mut self, config: &AccessPointConfig) -> Result<()> {
        let mut access_point = AccessPointConfiguration::default();
        access_point.ssid =
            config.ssid.as_str().try_into().map_err(|_| {
                anyhow::anyhow!("Could not parse the given AP SSID into WiFi config")
            })?;
        access_point.channel = config.channel;
        access_point.max_connections = config.max_connections.into();
        access_point.auth_method = match config.auth {
            AccessPointAuth::Open => AuthMethod::None,
        };

        self.wifi
            .set_configuration(&Configuration::AccessPoint(access_point))?;
        configure_softap_netif(self.wifi.wifi_mut(), &config.ip_config)?;
        self.wifi.start().await?;
        Ok(())
    }

    async fn stop_access_point(&mut self) -> Result<()> {
        self.wifi.stop().await?;
        Ok(())
    }

    async fn access_point_status(&mut self) -> Result<AccessPointStatus> {
        Ok(AccessPointStatus {
            is_started: self.wifi.is_started().unwrap_or(false),
            client_count: softap_client_count(),
        })
    }

    async fn access_point_ip_config(&mut self) -> Result<IpConfig> {
        let ip_info = self.wifi.wifi().ap_netif().get_ip_info()?;
        Ok(IpConfig::new(
            ip_info.ip.to_string(),
            ip_info.subnet.gateway.to_string(),
            ip_info.subnet.mask.to_string(),
        ))
    }
}

fn softap_client_count() -> usize {
    let mut list = sys::wifi_sta_list_t::default();
    unsafe {
        if sys::esp_wifi_ap_get_sta_list(&mut list as *mut _) == 0 {
            list.num as usize
        } else {
            0
        }
    }
}

fn configure_softap_netif(wifi: &mut EspWifi<'_>, config: &IpConfig) -> Result<()> {
    let handle = wifi.ap_netif_mut().handle();
    let mut ip_info = sys::esp_netif_ip_info_t {
        ip: ipv4_addr(&config.ip)?,
        gw: ipv4_addr(&config.gateway)?,
        netmask: ipv4_addr(&config.netmask)?,
    };

    unsafe {
        sys::esp_netif_dhcps_stop(handle);
    }
    esp_idf_svc::sys::esp!(unsafe { sys::esp_netif_set_ip_info(handle, &mut ip_info as *mut _) })?;
    esp_idf_svc::sys::esp!(unsafe { sys::esp_netif_dhcps_start(handle) })?;

    Ok(())
}

fn ipv4_addr(value: &str) -> Result<sys::esp_ip4_addr_t> {
    let octets: Vec<u8> = value
        .split('.')
        .map(|segment| {
            segment
                .parse::<u8>()
                .map_err(|_| anyhow::anyhow!("Invalid IPv4 address: {value}"))
        })
        .collect::<Result<Vec<_>>>()?;

    if octets.len() != 4 {
        return Err(anyhow::anyhow!("Invalid IPv4 address: {value}"));
    }

    Ok(sys::esp_ip4_addr_t {
        addr: u32::to_be(u32::from_be_bytes([
            octets[0], octets[1], octets[2], octets[3],
        ])),
    })
}
