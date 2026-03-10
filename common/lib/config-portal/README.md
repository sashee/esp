# config-portal

Reusable ESP-IDF helper for:

- declaring config fields from metadata
- checking stored NVS data against the declared schema
- rendering a small SoftAP-hosted configuration form
- saving or resetting NVS values

Current field kinds:

- text
- password

Typical usage:

```rust
use config_portal::{enter_error_mode, read_nvs, FieldSpec, NvsConfigState, PortalSpec};

static FIELDS: &[FieldSpec] = &[
    FieldSpec::text("ssid", "Wi-Fi SSID"),
    FieldSpec::password("pw", "Wi-Fi password"),
    FieldSpec::text("url", "Info panel URL"),
];

static SPEC: PortalSpec = PortalSpec {
    namespace: "config",
    ap_prefix: "InfoPanel",
    title: "Info Panel Setup",
    fields: FIELDS,
};

match read_nvs(&SPEC, nvs_partition.clone())? {
    NvsConfigState::Ready(config) => {
        let ssid = config.get("ssid").unwrap_or("");
        let pw = config.get("pw").unwrap_or("");
        let url = config.get("url").unwrap_or("");
        // run normal mode
    }
    NvsConfigState::Missing | NvsConfigState::SchemaMismatch => {
        enter_error_mode(&SPEC, "configuration required", &mut wifi, nvs_partition).await?;
    }
}
```

Behavior notes:

- SoftAP SSID is generated as `<prefix>-XXXX` from the device MAC address
- password fields are never pre-filled in `GET /`
- empty password submission keeps the previous stored password
- SoftAP netif is configured for `192.168.4.1`
- schema mismatch does not erase NVS until a successful save or explicit reset
