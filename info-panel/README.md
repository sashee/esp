# info-panel

ESP32-C6 Rust scaffold focused on Wi-Fi bring-up.

## Wi-Fi config

Create `cfg.toml` from `cfg.toml.example` and fill `known_wifis` in priority order.
Set `info_panel_url` to the full HTTP URL (including port and path), for example `/info-panel.rgb565`.
The firmware connects only to listed networks.
`cfg.toml` is compiled into the firmware at build time, so rebuild after changes.

## Build and flash

From `info-panel/`:

```bash
cargo run
```

If you use Docker for the toolchain, run the same command inside your container.
