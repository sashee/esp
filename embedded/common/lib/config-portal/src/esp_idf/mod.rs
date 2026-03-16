use anyhow::Result;
use embassy_time::{Duration, Instant, Timer};
use embedded_svc::{
    http::{server::Request, Headers, Method},
    io::{Read, Write},
};
use esp_idf_svc::{
    hal::task::block_on,
    http::server::{Configuration as HttpConfiguration, EspHttpServer},
    nvs::{EspNvs, EspNvsPartition, NvsPartitionId},
    sys::{self, ESP_ERR_NVS_NOT_FOUND},
};
use std::{collections::BTreeMap, future::Future, string::ToString, sync::Arc, vec, vec::Vec};

use crate::{
    ConfigClock, ConfigHttpBackend, ConfigPlatform, ConfigStore, HttpEndpoint, HttpMethod,
    HttpRequest, HttpResponse,
};

pub struct EspHttpBackend;

impl EspHttpBackend {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EspHttpBackend {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EspPlatform;

impl EspPlatform {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EspPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigPlatform for EspPlatform {
    fn mac_address(&self) -> Result<[u8; 6]> {
        let mut mac = [0_u8; 6];
        esp_idf_svc::sys::esp!(unsafe { sys::esp_efuse_mac_get_default(mac.as_mut_ptr()) })?;
        Ok(mac)
    }

    fn reboot(&self) -> ! {
        unsafe {
            sys::esp_restart();
        }
    }
}

pub struct EspClock;

impl EspClock {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EspClock {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigClock for EspClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    async fn sleep(&self, duration: Duration) {
        Timer::after(duration).await;
    }
}

impl ConfigHttpBackend for EspHttpBackend {
    type Server = EspHttpServer<'static>;

    fn start<H, Fut>(self, endpoints: &'static [HttpEndpoint], handler: H) -> Result<Self::Server>
    where
        H: Fn(HttpRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HttpResponse>> + Send,
    {
        let mut server = EspHttpServer::new(&HttpConfiguration::default())?;
        let handler = Arc::new(handler);

        for endpoint in endpoints {
            let endpoint = *endpoint;
            let handler = handler.clone();

            server.fn_handler::<anyhow::Error, _>(
                endpoint.path,
                esp_method(endpoint.method)?,
                move |mut request| {
                    let response = block_on(handler(build_request(endpoint, &mut request)?))?;
                    let mut raw_response = request.into_response(
                        response.status_code,
                        None,
                        &[("Content-Type", response.content_type)],
                    )?;
                    raw_response.write_all(&response.body)?;
                    Ok(())
                },
            )?;
        }

        Ok(server)
    }
}

pub struct NvsConfigStore<T>
where
    T: NvsPartitionId,
{
    partition: EspNvsPartition<T>,
    namespace: &'static str,
}

impl<T> Clone for NvsConfigStore<T>
where
    T: NvsPartitionId,
{
    fn clone(&self) -> Self {
        Self {
            partition: self.partition.clone(),
            namespace: self.namespace,
        }
    }
}

impl<T> NvsConfigStore<T>
where
    T: NvsPartitionId,
{
    pub fn new(partition: EspNvsPartition<T>, namespace: &'static str) -> Self {
        Self {
            partition,
            namespace,
        }
    }
}

impl<T> ConfigStore for NvsConfigStore<T>
where
    T: NvsPartitionId,
{
    fn read(&self, keys: &[&str]) -> Result<BTreeMap<String, String>> {
        let nvs = match EspNvs::new(self.partition.clone(), self.namespace, false) {
            Ok(nvs) => nvs,
            Err(err) if err.code() == ESP_ERR_NVS_NOT_FOUND => return Ok(BTreeMap::new()),
            Err(err) => return Err(err.into()),
        };

        let mut values = BTreeMap::new();
        for key in keys {
            if let Some(value) = read_string(&nvs, key)? {
                values.insert((*key).to_string(), value);
            }
        }

        Ok(values)
    }

    fn write(&self, values: &BTreeMap<String, String>) -> Result<()> {
        let nvs = EspNvs::new(self.partition.clone(), self.namespace, true)?;
        for (key, value) in values {
            nvs.set_str(key, value)?;
        }
        Ok(())
    }

    fn remove(&self, keys: &[&str]) -> Result<()> {
        let nvs = EspNvs::new(self.partition.clone(), self.namespace, true)?;
        for key in keys {
            let _ = nvs.remove(key)?;
        }
        Ok(())
    }
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

fn build_request<T>(endpoint: HttpEndpoint, request: &mut Request<&mut T>) -> Result<HttpRequest>
where
    T: embedded_svc::http::server::Connection,
    T::Error: core::fmt::Debug,
{
    Ok(HttpRequest {
        method: parse_method(endpoint.method),
        path: endpoint.path.to_string(),
        headers: collect_headers(request),
        body: read_body(request)?,
    })
}

fn esp_method(method: &str) -> Result<Method> {
    match method {
        "GET" => Ok(Method::Get),
        "POST" => Ok(Method::Post),
        _ => Err(anyhow::anyhow!("unsupported HTTP method: {method}")),
    }
}

fn parse_method(method: &str) -> HttpMethod {
    match method {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        other => HttpMethod::Other(other.to_string()),
    }
}

fn collect_headers(headers: &impl Headers) -> BTreeMap<String, String> {
    let mut collected = BTreeMap::new();
    for name in [
        "Host",
        "User-Agent",
        "Accept",
        "Accept-Encoding",
        "Accept-Language",
        "Connection",
        "Referer",
        "Cookie",
        "Content-Type",
    ] {
        if let Some(value) = headers.header(name) {
            collected.insert(name.to_string(), value.to_string());
        }
    }
    collected
}

fn read_body<T>(request: &mut T) -> Result<Vec<u8>>
where
    T: Read,
    <T as embedded_svc::io::ErrorType>::Error: core::fmt::Debug,
{
    let mut body = Vec::new();
    let mut buf = [0_u8; 256];

    loop {
        let read = request
            .read(&mut buf)
            .map_err(|err| anyhow::anyhow!("failed reading request body: {err:?}"))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buf[..read]);
    }

    Ok(body)
}
