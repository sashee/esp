use std::{env, fs, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct KnownWifiCfg {
    ssid: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct AppConfig {
    #[serde(default)]
    known_wifis: Vec<KnownWifiCfg>,
    #[serde(default)]
    #[serde(alias = "png_url")]
    info_panel_url: String,
}

#[derive(Debug, Deserialize)]
struct RootConfig {
    #[serde(rename = "info-panel")]
    info_panel: AppConfig,
}

fn escape_rust_string(input: &str) -> String {
    input
        .chars()
        .flat_map(|c| c.escape_default())
        .collect::<String>()
}

fn main() {
    embuild::espidf::sysenv::output();

    println!("cargo:rerun-if-changed=cfg.toml");
    println!("cargo:rerun-if-changed=cfg.toml.example");

    let cfg_raw = fs::read_to_string("cfg.toml")
        .expect("missing cfg.toml; copy cfg.toml.example to cfg.toml and fill known_wifis");
    let parsed: RootConfig = toml::from_str(&cfg_raw).expect(
        "invalid cfg.toml; expected [info-panel].known_wifis and [info-panel].info_panel_url",
    );

    let mut out = String::from("const KNOWN_WIFIS: &[KnownWifi] = &[\n");
    for entry in &parsed.info_panel.known_wifis {
        out.push_str("    KnownWifi { ssid: \"");
        out.push_str(&escape_rust_string(&entry.ssid));
        out.push_str("\", password: \"");
        out.push_str(&escape_rust_string(&entry.password));
        out.push_str("\" },\n");
    }
    out.push_str("];\n");
    out.push_str("const INFO_PANEL_URL: &str = \"");
    out.push_str(&escape_rust_string(&parsed.info_panel.info_panel_url));
    out.push_str("\";\n");

    let out_path =
        PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set")).join("wifi_config.rs");
    fs::write(out_path, out).expect("failed to write generated wifi_config.rs");
}
