# info-panel

ESP32-C6 Rust scaffold focused on Wi-Fi bring-up.

## Runtime config

The firmware reads `ssid`, `pw`, and `url` from NVS at boot.
There is no longer any build-time `cfg.toml` for Wi-Fi or URL configuration.

If config is missing, mismatched, or normal mode fails, the device starts a temporary SoftAP on `192.168.4.1`.
The AP name is generated as `InfoPanel-XXXX` from the device MAC address.

Portal behavior:

- `GET /` shows the current `ssid` and `url`
- password is never pre-filled
- empty password submit keeps the existing stored password
- `POST /reset` clears stored config and reboots
- no station connected: reboot after 1 minute
- any station connected: reboot after 10 minutes total

## Build and flash

From `info-panel/`:

```bash
cargo run
```

If you use Docker for the toolchain, run the same command inside your container.
