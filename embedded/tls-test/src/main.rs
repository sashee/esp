use anyhow::Result;
use libcrux_ml_kem::{
    mlkem768,
    KEY_GENERATION_SEED_SIZE, SHARED_SECRET_SIZE,
};
use log::info;
use config_portal::{esp_idf::NvsConfigStore, ConfigStore};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{peripherals::Peripherals, task::block_on},
    nvs::EspDefaultNvsPartition,
    sntp::{EspSntp, SyncStatus},
};
use rustls::{
    crypto::{ActiveKeyExchange, CompletedKeyExchange, SharedSecret, SupportedKxGroup},
    pki_types::ServerName,
    ClientConfig, ClientConnection, NamedGroup, ProtocolVersion, RootCertStore, StreamOwned,
};
use std::{
    fmt,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use wifi::{ConnectState, Wifi, WifiCredentials};

use wifi::esp_idf::EspWifiBackend;

const CONFIG_NAMESPACE: &str = "config";
const WIFI_KEYS: &[&str] = &["ssid", "pw"];
const TCP_TEST_TARGET: &str = "1.1.1.1:443";
const TCP_TEST_TIMEOUT: Duration = Duration::from_secs(10);
const TLS_TEST_SERVER_NAME: &str = "one.one.one.one";
const TIME_SYNC_TIMEOUT: Duration = Duration::from_secs(20);
const MIN_VALID_UNIX_TIME: u64 = 1_735_689_600;
const LIBCRUX_TEST_STACK_SIZE: usize = 96 * 1024;
const TLS_TEST_STACK_SIZE: usize = 128 * 1024;
const HTTP_RESPONSE_PREVIEW_LIMIT: usize = 1024;

static LIBCRUX_MLKEM768: &dyn SupportedKxGroup = &LibcruxMlKem768;
static X25519_MLKEM768: &dyn SupportedKxGroup = &HybridX25519MlKem768;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    rustls_rustcrypto::provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("failed to install rustls crypto provider"))?;

    block_on(async_main())
}

async fn async_main() -> Result<()> {
    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let store = NvsConfigStore::new(nvs.clone());

    let credentials = read_wifi_credentials(&store)?;
    let wifi_backend = EspWifiBackend::new_with_default_nvs(peripherals.modem, sysloop, Some(nvs))?;
    let mut wifi = Wifi::new(wifi_backend);

    info!("tls-test booted");
    info!("connecting to Wi-Fi SSID '{}'", credentials.ssid);

    let connection = wifi.connect(&credentials, log_connect_state).await?;
    info!("Wi-Fi connected with IP {}", connection.ip);

    let _sntp = sync_time()?;

    run_libcrux_mlkem_test()?;

    run_tcp_test()?;
    run_tls_handshake_test()?;

    loop {
        info!("tls-test heartbeat");
        thread::sleep(Duration::from_secs(5));
    }
}

fn run_tcp_test() -> Result<()> {
    let addr: SocketAddr = TCP_TEST_TARGET.parse()?;

    info!("TCP test connecting to {}", addr);

    let stream = TcpStream::connect_timeout(&addr, TCP_TEST_TIMEOUT)?;
    stream.set_read_timeout(Some(TCP_TEST_TIMEOUT))?;
    stream.set_write_timeout(Some(TCP_TEST_TIMEOUT))?;

    info!("TCP test connected to {}", addr);

    Ok(())
}

fn run_libcrux_mlkem_test() -> Result<()> {
    info!("libcrux ML-KEM test starting");

    thread::Builder::new()
        .stack_size(LIBCRUX_TEST_STACK_SIZE)
        .spawn(run_libcrux_mlkem_test_inner)?
        .join()
        .map_err(|_| anyhow::anyhow!("libcrux ML-KEM test thread panicked"))?
}

fn run_libcrux_mlkem_test_inner() -> Result<()> {
    let client = LIBCRUX_MLKEM768.start()?;
    let client_share_len = client.pub_key().len();
    let server = LIBCRUX_MLKEM768.start_and_complete(client.pub_key())?;
    let server_share_len = server.pub_key.len();
    let client_secret = client.complete(&server.pub_key)?;

    anyhow::ensure!(
        client_secret.secret_bytes() == server.secret.secret_bytes(),
        "libcrux ML-KEM shared secret mismatch"
    );

    info!(
        "libcrux ML-KEM roundtrip ok: group={:?} client_share_len={} server_share_len={} secret_len={}",
        server.group,
        client_share_len,
        server_share_len,
        server.secret.secret_bytes().len()
    );

    Ok(())
}

fn sync_time() -> Result<EspSntp<'static>> {
    info!("starting SNTP time sync");

    let sntp = EspSntp::new_default()?;
    let deadline = SystemTime::now() + TIME_SYNC_TIMEOUT;
    let mut last_status = None;

    loop {
        let status = sntp.get_sync_status();
        if last_status != Some(status) {
            info!("SNTP sync status: {:?}", status);
            last_status = Some(status);
        }

        let unix_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if unix_time >= MIN_VALID_UNIX_TIME {
            info!("system time synced: unix_time={}", unix_time);
            return Ok(sntp);
        }

        anyhow::ensure!(
            SystemTime::now() < deadline,
            "timed out waiting for SNTP sync; last_status={:?} unix_time={}",
            status,
            unix_time
        );

        if matches!(status, SyncStatus::Reset | SyncStatus::InProgress) {
            thread::sleep(Duration::from_millis(250));
            continue;
        }

        thread::sleep(Duration::from_millis(250));
    }
}

fn run_tls_handshake_test() -> Result<()> {
    thread::Builder::new()
        .stack_size(TLS_TEST_STACK_SIZE)
        .spawn(run_tls_handshake_test_inner)?
        .join()
        .map_err(|_| anyhow::anyhow!("TLS handshake test thread panicked"))?
}

fn run_tls_handshake_test_inner() -> Result<()> {
    let addr: SocketAddr = TCP_TEST_TARGET.parse()?;
    let server_name = ServerName::try_from(TLS_TEST_SERVER_NAME.to_owned())?;
    let root_store = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut provider = rustls_rustcrypto::provider();
    provider.kx_groups = vec![X25519_MLKEM768];
    let config = Arc::new(
        ClientConfig::builder_with_provider(Arc::new(provider))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );
    let mut stream = TcpStream::connect_timeout(&addr, TCP_TEST_TIMEOUT)?;

    stream.set_read_timeout(Some(TCP_TEST_TIMEOUT))?;
    stream.set_write_timeout(Some(TCP_TEST_TIMEOUT))?;

    let mut tls = ClientConnection::new(config, server_name)?;

    info!(
        "TLS test handshaking with {} via {}",
        TLS_TEST_SERVER_NAME, addr
    );

    while tls.is_handshaking() {
        if let Err(err) = tls.complete_io(&mut stream) {
            let key_exchange = tls
                .negotiated_key_exchange_group()
                .map(|group| format!("{:?}", group.name()))
                .unwrap_or_else(|| "unknown".to_string());
            info!(
                "TLS handshake failed: key_exchange={} error={}",
                key_exchange, err
            );
            return Err(err.into());
        }
    }

    let protocol = tls
        .protocol_version()
        .map(|version| format!("{version:?}"))
        .unwrap_or_else(|| "unknown".to_string());
    let key_exchange = tls
        .negotiated_key_exchange_group()
        .map(|group| format!("{:?}", group.name()))
        .unwrap_or_else(|| "unknown".to_string());
    let cipher_suite = tls
        .negotiated_cipher_suite()
        .map(|suite| format!("{:?}", suite.suite()))
        .unwrap_or_else(|| "unknown".to_string());

    info!(
        "TLS handshake complete: version={} key_exchange={} cipher_suite={}",
        protocol, key_exchange, cipher_suite
    );

    run_https_get(StreamOwned::new(tls, stream))?;

    Ok(())
}

fn run_https_get(mut tls_stream: StreamOwned<ClientConnection, TcpStream>) -> Result<()> {
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: tls-test/0.1\r\n\r\n",
        TLS_TEST_SERVER_NAME,
    );

    info!("HTTPS GET sending request to {}", TLS_TEST_SERVER_NAME);
    tls_stream.write_all(request.as_bytes())?;
    tls_stream.flush()?;

    let mut preview = Vec::with_capacity(HTTP_RESPONSE_PREVIEW_LIMIT);
    let mut chunk = [0u8; 256];

    while preview.len() < HTTP_RESPONSE_PREVIEW_LIMIT {
        let bytes_read = tls_stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }

        let remaining = HTTP_RESPONSE_PREVIEW_LIMIT - preview.len();
        preview.extend_from_slice(&chunk[..bytes_read.min(remaining)]);
    }

    anyhow::ensure!(!preview.is_empty(), "HTTPS GET returned no response bytes");

    let preview_text = String::from_utf8_lossy(&preview);
    let status_line = preview_text.lines().next().unwrap_or("<missing status line>");

    info!("HTTPS GET status: {}", status_line);
    info!("HTTPS GET preview:\n{}", preview_text);

    Ok(())
}

struct LibcruxMlKem768;
struct HybridX25519MlKem768;
struct LocalX25519;

#[derive(Clone, Copy)]
struct HybridLayout {
    classical_share_len: usize,
    post_quantum_client_share_len: usize,
    post_quantum_server_share_len: usize,
    post_quantum_first: bool,
}

const X25519_MLKEM768_LAYOUT: HybridLayout = HybridLayout {
    classical_share_len: 32,
    post_quantum_client_share_len: 1184,
    post_quantum_server_share_len: 1088,
    post_quantum_first: true,
};

impl fmt::Debug for LibcruxMlKem768 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MLKEM768")
    }
}

impl fmt::Debug for HybridX25519MlKem768 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("X25519MLKEM768")
    }
}

impl fmt::Debug for LocalX25519 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("X25519")
    }
}

impl SupportedKxGroup for HybridX25519MlKem768 {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        let classical = LocalX25519.start()?;
        let post_quantum = LIBCRUX_MLKEM768.start()?;
        let combined_pub_key = X25519_MLKEM768_LAYOUT.concat(post_quantum.pub_key(), classical.pub_key());

        Ok(Box::new(ActiveHybrid {
            classical,
            post_quantum,
            combined_pub_key,
        }))
    }

    fn start_and_complete(&self, client_share: &[u8]) -> Result<CompletedKeyExchange, rustls::Error> {
        let (post_quantum_share, classical_share) = X25519_MLKEM768_LAYOUT
            .split_received_client_share(client_share)
            .ok_or(rustls::Error::PeerMisbehaved(rustls::PeerMisbehaved::InvalidKeyShare))?;
        let classical = LocalX25519.start_and_complete(classical_share)?;
        let post_quantum = LIBCRUX_MLKEM768.start_and_complete(post_quantum_share)?;

        Ok(CompletedKeyExchange {
            group: self.name(),
            pub_key: X25519_MLKEM768_LAYOUT.concat(&post_quantum.pub_key, &classical.pub_key),
            secret: SharedSecret::from(X25519_MLKEM768_LAYOUT.concat(
                post_quantum.secret.secret_bytes(),
                classical.secret.secret_bytes(),
            )),
        })
    }

    fn ffdhe_group(&self) -> Option<rustls::ffdhe_groups::FfdheGroup<'static>> {
        None
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519MLKEM768
    }

    fn usable_for_version(&self, version: ProtocolVersion) -> bool {
        version == ProtocolVersion::TLSv1_3
    }
}

impl SupportedKxGroup for LocalX25519 {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        let private_key = x25519_dalek::StaticSecret::from(fill_random::<32>()?);
        let public_key = x25519_dalek::PublicKey::from(&private_key);

        Ok(Box::new(ActiveLocalX25519 {
            private_key,
            public_key: public_key.as_bytes().to_vec(),
        }))
    }

    fn ffdhe_group(&self) -> Option<rustls::ffdhe_groups::FfdheGroup<'static>> {
        None
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::X25519
    }

    fn usable_for_version(&self, version: ProtocolVersion) -> bool {
        version == ProtocolVersion::TLSv1_3
    }
}

impl SupportedKxGroup for LibcruxMlKem768 {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, rustls::Error> {
        let key_pair = mlkem768::generate_key_pair(fill_random::<KEY_GENERATION_SEED_SIZE>()?);

        Ok(Box::new(ActiveLibcruxMlKem768 {
            private_key: (*key_pair.sk()).into(),
            public_key: key_pair.pk().to_vec(),
        }))
    }

    fn start_and_complete(&self, peer_pub_key: &[u8]) -> Result<CompletedKeyExchange, rustls::Error> {
        let public_key = parse_mlkem768_public_key(peer_pub_key)?;
        let (ciphertext, secret) = mlkem768::encapsulate(&public_key, fill_random::<SHARED_SECRET_SIZE>()?);

        Ok(CompletedKeyExchange {
            group: self.name(),
            pub_key: ciphertext.as_slice().to_vec(),
            secret: SharedSecret::from(secret.as_slice()),
        })
    }

    fn ffdhe_group(&self) -> Option<rustls::ffdhe_groups::FfdheGroup<'static>> {
        None
    }

    fn name(&self) -> NamedGroup {
        NamedGroup::MLKEM768
    }

    fn usable_for_version(&self, version: ProtocolVersion) -> bool {
        version == ProtocolVersion::TLSv1_3
    }
}

struct ActiveLibcruxMlKem768 {
    private_key: mlkem768::MlKem768PrivateKey,
    public_key: Vec<u8>,
}

struct ActiveLocalX25519 {
    private_key: x25519_dalek::StaticSecret,
    public_key: Vec<u8>,
}

struct ActiveHybrid {
    classical: Box<dyn ActiveKeyExchange>,
    post_quantum: Box<dyn ActiveKeyExchange>,
    combined_pub_key: Vec<u8>,
}

impl ActiveKeyExchange for ActiveLibcruxMlKem768 {
    fn complete(self: Box<Self>, peer_pub_key: &[u8]) -> Result<SharedSecret, rustls::Error> {
        let ciphertext = parse_mlkem768_ciphertext(peer_pub_key)?;
        let secret = mlkem768::decapsulate(&self.private_key, &ciphertext);

        Ok(SharedSecret::from(secret.as_slice()))
    }

    fn ffdhe_group(&self) -> Option<rustls::ffdhe_groups::FfdheGroup<'static>> {
        None
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::MLKEM768
    }

    fn pub_key(&self) -> &[u8] {
        &self.public_key
    }
}

impl ActiveKeyExchange for ActiveLocalX25519 {
    fn complete(self: Box<Self>, peer_pub_key: &[u8]) -> Result<SharedSecret, rustls::Error> {
        let peer_pub_key: [u8; 32] = peer_pub_key
            .try_into()
            .map_err(|_| rustls::Error::PeerMisbehaved(rustls::PeerMisbehaved::InvalidKeyShare))?;
        let peer_public = x25519_dalek::PublicKey::from(peer_pub_key);

        Ok(SharedSecret::from(
            self.private_key.diffie_hellman(&peer_public).as_bytes().as_slice(),
        ))
    }

    fn ffdhe_group(&self) -> Option<rustls::ffdhe_groups::FfdheGroup<'static>> {
        None
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::X25519
    }

    fn pub_key(&self) -> &[u8] {
        &self.public_key
    }
}

impl ActiveKeyExchange for ActiveHybrid {
    fn complete(self: Box<Self>, peer_pub_key: &[u8]) -> Result<SharedSecret, rustls::Error> {
        let (post_quantum_share, classical_share) = X25519_MLKEM768_LAYOUT
            .split_received_server_share(peer_pub_key)
            .ok_or(rustls::Error::PeerMisbehaved(rustls::PeerMisbehaved::InvalidKeyShare))?;
        let classical = self.classical.complete(classical_share)?;
        let post_quantum = self.post_quantum.complete(post_quantum_share)?;

        Ok(SharedSecret::from(X25519_MLKEM768_LAYOUT.concat(
            post_quantum.secret_bytes(),
            classical.secret_bytes(),
        )))
    }

    fn hybrid_component(&self) -> Option<(NamedGroup, &[u8])> {
        Some((self.classical.group(), self.classical.pub_key()))
    }

    fn complete_hybrid_component(self: Box<Self>, peer_pub_key: &[u8]) -> Result<SharedSecret, rustls::Error> {
        self.classical.complete(peer_pub_key)
    }

    fn ffdhe_group(&self) -> Option<rustls::ffdhe_groups::FfdheGroup<'static>> {
        None
    }

    fn group(&self) -> NamedGroup {
        NamedGroup::X25519MLKEM768
    }

    fn pub_key(&self) -> &[u8] {
        &self.combined_pub_key
    }
}

impl HybridLayout {
    fn split_received_client_share<'a>(&self, share: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
        self.split(share, self.post_quantum_client_share_len)
    }

    fn split_received_server_share<'a>(&self, share: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
        self.split(share, self.post_quantum_server_share_len)
    }

    fn split<'a>(&self, share: &'a [u8], post_quantum_share_len: usize) -> Option<(&'a [u8], &'a [u8])> {
        if share.len() != self.classical_share_len + post_quantum_share_len {
            return None;
        }

        Some(if self.post_quantum_first {
            let (post_quantum, classical) = share.split_at(post_quantum_share_len);
            (post_quantum, classical)
        } else {
            let (classical, post_quantum) = share.split_at(self.classical_share_len);
            (post_quantum, classical)
        })
    }

    fn concat(&self, post_quantum: &[u8], classical: &[u8]) -> Vec<u8> {
        if self.post_quantum_first {
            [post_quantum, classical].concat()
        } else {
            [classical, post_quantum].concat()
        }
    }
}

fn fill_random<const N: usize>() -> Result<[u8; N], rustls::Error> {
    let mut bytes = [0u8; N];

    unsafe {
        esp_idf_svc::sys::esp_fill_random(bytes.as_mut_ptr().cast(), bytes.len());
    }

    Ok(bytes)
}

fn parse_mlkem768_public_key(peer_pub_key: &[u8]) -> Result<mlkem768::MlKem768PublicKey, rustls::Error> {
    let key_bytes: [u8; 1184] = peer_pub_key
        .try_into()
        .map_err(|_| rustls::Error::PeerMisbehaved(rustls::PeerMisbehaved::InvalidKeyShare))?;
    let public_key = key_bytes.into();

    if !mlkem768::validate_public_key(&public_key) {
        return Err(rustls::Error::PeerMisbehaved(
            rustls::PeerMisbehaved::InvalidKeyShare,
        ));
    }

    Ok(public_key)
}

fn parse_mlkem768_ciphertext(peer_pub_key: &[u8]) -> Result<mlkem768::MlKem768Ciphertext, rustls::Error> {
    let ciphertext: [u8; 1088] = peer_pub_key
        .try_into()
        .map_err(|_| rustls::Error::PeerMisbehaved(rustls::PeerMisbehaved::InvalidKeyShare))?;

    Ok(ciphertext.into())
}

fn read_wifi_credentials<S>(store: &S) -> Result<WifiCredentials>
where
    S: ConfigStore,
{
    let config = store.read(CONFIG_NAMESPACE, WIFI_KEYS)?;
    let ssid = config.get("ssid").cloned().unwrap_or_default();
    let password = config.get("pw").cloned().unwrap_or_default();

    if ssid.is_empty() {
        anyhow::bail!(
            "missing Wi-Fi config in NVS namespace '{}' key 'ssid'",
            CONFIG_NAMESPACE
        );
    }

    Ok(WifiCredentials::new(ssid, password))
}

fn log_connect_state(state: ConnectState) {
    match state {
        ConnectState::Starting => info!("Wi-Fi starting"),
        ConnectState::Scanning => info!("Wi-Fi scanning"),
        ConnectState::ScanComplete { networks_found } => {
            info!("Wi-Fi scan complete: {} networks found", networks_found)
        }
        ConnectState::Configuring {
            ssid,
            channel,
            auth,
        } => info!(
            "Wi-Fi configuring ssid='{}' channel={:?} auth={:?}",
            ssid, channel, auth
        ),
        ConnectState::Connecting => info!("Wi-Fi connecting"),
        ConnectState::WaitingForIp => info!("Wi-Fi waiting for DHCP"),
        ConnectState::Connected { ip } => info!("Wi-Fi connected: {}", ip),
    }
}
