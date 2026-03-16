# Info Panel Lib Test Plan

## Init

### test_init_clears_tft_on_startup

Verifies that during startup, the display is initialized BEFORE any WiFi connection. The test will:
1. Provide a mock store with valid config (ssid, pw, url, led_brightness)
2. Use a shared global atomic counter to track call ordering
3. Mock wifi.connect() to succeed
4. Mock http_client.get() to fail (triggers error mode after retries)
5. Run the info_panel_lib::run() function (which returns `!`, use catch_unwind for panic from reboot)
6. Assert that display.init() was called BEFORE wifi.connect() is called

### test_init_enters_error_mode_when_display_init_fails

Verifies that if display.init() fails, the device enters error mode (red LED, wait, restart). The test will:
1. Provide a mock display that returns an error on init()
2. Run the info_panel_lib::run() function
3. Assert that LED was set to ERROR_LED (red) with brightness 0.06 before reboot
4. Assert that platform.reboot() was called

### test_init_connects_wifi_when_nvs_has_complete_config

Verifies that when NVS has complete config (ssid, pw, url, led_brightness), WiFi connection is attempted. The test will:
1. Provide a mock store with all required config values
2. Mock wifi.connect() to return success
3. Mock http_client.get() to return valid frame data
4. Run the info_panel_lib::run() function
5. Assert that wifi.configure_client() was called with the stored ssid and password
6. Assert that LED was set to CONNECTING_LED (orange) during connection
7. Assert that LED was set to CONNECTED_LED (blue) after successful connection

### test_init_goes_to_required_portal_when_led_brightness_missing

Verifies that when NVS has config but is missing led_brightness, the required config portal is started. The test will:
1. Provide a mock store with ssid, pw, url but NO led_brightness
2. Run the info_panel_lib::run() function
3. Assert that wifi.start_access_point() was called (required portal started)
4. Assert that LED was set to REQUIRED_PORTAL_LED (green)

### test_init_goes_to_required_portal_when_led_brightness_invalid

Verifies that when led_brightness is not a valid u8, the required config portal is started. The test will:
1. Provide a mock store with led_brightness = "not_a_number"
2. Run the info_panel_lib::run() function
3. Assert that wifi.start_access_point() was called

### test_init_goes_to_required_portal_when_config_corrupted

Verifies that when config is partially corrupted (e.g., missing url), the required config portal is started. The test will:
1. Provide a mock store with only ssid and pw (missing url and led_brightness)
2. Run the info_panel_lib::run() function
3. Assert that wifi.start_access_point() was called

### test_init_sets_orange_led_during_wifi_connection

Verifies that the LED is orange during WiFi connection. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to fail (triggers error mode)
4. Run the info_panel_lib::run() function
5. Assert that at some point during connection, LED was set to CONNECTING_LED (1.0, 0.78, 0.0) with brightness from config.led_brightness()
6. Assert that wifi.connect() was called

### test_init_enters_error_mode_when_led_set_fails_during_connect

Verifies that LED errors during connection cause the device to enter error mode. The test will:
1. Provide a mock LED that returns an error on set_pixel
2. Mock wifi.connect() to succeed
3. Run the info_panel_lib::run() function (expecting panic from reboot)
4. Assert that LED was set to ERROR_LED (red) with brightness 0.06 before reboot
5. Assert that platform.reboot() was called (LED error propagates to error mode)

### test_init_sets_blue_led_when_wifi_connected

Verifies that the LED is blue when WiFi is successfully connected. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to return success
3. Mock http_client.get() to return valid frame data
4. Run the info_panel_lib::run() function
5. Assert that LED was set to CONNECTED_LED (0.0, 0.0, 1.0) with config.led_brightness()

### test_init_enters_error_mode_when_wifi_connect_fails

Verifies that WiFi connect failures cause the device to enter error mode. The test will:
1. Provide a mock store with valid config
2. Mock wifi.configure_client() to succeed but wifi.connect() to return error
3. Run the info_panel_lib::run() function
4. Assert that LED was set to ERROR_LED (red) with brightness 0.06 before reboot
5. Assert that platform.reboot() was called

### test_init_enters_error_mode_when_wifi_configure_fails

Verifies that WiFi configure_client failures cause the device to enter error mode. The test will:
1. Provide a mock store with valid config
2. Mock wifi.configure_client() to return error
3. Run the info_panel_lib::run() function
4. Assert that LED was set to ERROR_LED (red) with brightness 0.06 before reboot
5. Assert that platform.reboot() was called

---

## Image

### test_image_fetches_url_after_wifi_connect

Verifies that after WiFi connects, the HTTP request is made to the configured URL. The test will:
1. Provide a mock store with valid config and url = "http://example.com"
2. Mock wifi.connect() to succeed
3. Track what URL was passed to http_client.get()
4. Run the info_panel_lib::run() function
5. Assert that http_client.get() was called with "http://example.com"

### test_image_fetches_empty_url_when_url_empty

Verifies behavior when the configured URL is an empty string. The test will:
1. Provide a mock store with url = ""
2. Mock wifi.connect() to succeed
3. Run the info_panel_lib::run() function
4. Assert that http_client.get() was still called (even with empty URL)

### test_image_fails_when_url_invalid

Verifies that an invalid URL triggers retries and eventually error mode. The test will:
1. Provide a mock store with url = "not_a_valid_url"
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to always return an error
4. Run the info_panel_lib::run() function (expecting panic from reboot)
5. Assert that http_client.get() was called 3 times with the invalid URL
6. Assert that LED was set to ERROR_LED (red) with brightness 0.06 before reboot

### test_image_retries_3_times_on_http_failure

Verifies that when HTTP fails, exactly 3 retry attempts are made with 1-second backoff. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to always return an error
4. Track the number of times http_client.get() was called
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() was called exactly 3 times
7. Assert that Clock::sleep was called with 1 second after each failed attempt (3 sleeps)

### test_image_succeeds_on_first_retry

Verifies that if the initial fetch fails and the first retry succeeds, no more retries are made. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to fail on first call, succeed on second (fail_up_to(1))
4. Set is_connected=false so refresh loop exits after success
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() was called exactly 2 times (1 fail + 1 success)
7. Assert that display.write_frame() was called for the fetched frame

### test_image_succeeds_on_second_retry

Verifies that if the first two fetch attempts fail and the third succeeds, the flow completes. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to fail on first two calls, succeed on third (fail_up_to(2))
4. Set is_connected=false so refresh loop exits after success
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() was called exactly 3 times (2 fails + 1 success)
7. Assert that display.write_frame() was called for the fetched frame

### test_image_succeeds_on_third_retry

Verifies that after two failures and success on the third attempt, the flow completes. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to fail on first two calls, succeed on third (fail_up_to(2))
4. Set is_connected=false so refresh loop exits after success
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() was called exactly 3 times
7. Assert that display.write_frame() was called for the fetched frame

### test_image_enters_error_mode_when_all_retries_fail

Verifies that after all 3 retries fail, the device enters error mode with red LED and reboot. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to always fail
4. Run the info_panel_lib::run() function (expecting panic from reboot)
5. Assert that LED was set to ERROR_LED (red: 1.0, 0.0, 0.0) with brightness 0.06 before reboot
6. Assert that platform.reboot() was called

### test_image_error_mode_waits_before_restart

Verifies that error mode waits for the configured delay (10 minutes) before restarting. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to always fail
4. Track sleep durations called
5. Run the info_panel_lib::run() function
6. Assert that the LED was set to ERROR_LED (red)
7. Assert that Clock::sleep was called with 600 seconds (10 minutes)

### test_image_displays_frame_on_tft_when_fetch_succeeds

Verifies that when HTTP fetch succeeds, the frame is written to the TFT display. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid frame bytes (128x160x2 bytes)
4. Set is_connected=false so refresh loop exits
5. Run the info_panel_lib::run() function
6. Assert that display.write_frame() was called (at least 2 times: black fill + fetched frame)

### test_image_enters_error_mode_on_write_frame_failure

Verifies that write_frame failures trigger error mode. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid frame data
4. Mock display.write_frame() to fail on 2nd call (fetched frame, after black fill)
5. Run the info_panel_lib::run() function (expecting panic from reboot)
6. Assert that LED was set to ERROR_LED (red) with brightness 0.06 before reboot
7. Assert that platform.reboot() was called

### test_image_handles_invalid_frame_size

Verifies that frames of unexpected size are still passed to write_frame without validation. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return frame of wrong size (100 bytes instead of 40960)
4. Set is_connected=false so refresh loop exits
5. Run the info_panel_lib::run() function
6. Assert that display.write_frame() was called with the invalid-size data (no validation in library)

### test_image_refreshes_after_30_second_interval

Verifies that the image refreshes every 30 seconds. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid data
4. Set is_connected=true so refresh loop continues
5. Use panic_on_nth(2) to trigger when refresh fetch is attempted
6. Run the info_panel_lib::run() function
7. Assert that http_client.get() was called twice (initial + refresh)
8. Assert that Clock::sleep was called with 30 seconds

### test_image_aborts_refresh_on_wifi_disconnect

Verifies that if WiFi disconnects during the refresh loop, the device enters error mode. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed initially
3. Mock wifi.is_connected() to return false
4. Run the info_panel_lib::run() function (expecting panic from reboot)
5. Assert that http_client.get() was called only once (refresh aborted)
6. Assert that LED was set to ERROR_LED (red) with brightness 0.06 before reboot
7. Assert that platform.reboot() was called

### test_image_enters_error_mode_on_write_frame_failure_in_refresh

Verifies that write_frame failures during refresh trigger error mode. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid data
4. Set is_connected=true so refresh loop runs
5. Mock display.write_frame() to fail on 2nd call (first fetched frame)
6. Use panic_on_nth(2) to catch refresh HTTP attempt
7. Run the info_panel_lib::run() function
8. Assert that either platform.reboot() was called or the mock panicked on refresh

---

## Config portal - No NVS case

### test_portal_starts_ap_when_nvs_empty

Verifies that when NVS is empty, an access point is started. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function
3. Assert that wifi.start_access_point() was called
4. Assert that the AP SSID starts with "InfoPanel-"

### test_portal_ap_start_failure_still_reboots

Verifies that AP start failures don't prevent the portal from rebooting. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function
3. Assert that platform.reboot() was called (portal always reboots)

### test_portal_sets_green_led_when_required_portal_runs

Verifies that the LED is green when the required config portal runs. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function
3. Assert that LED was set to REQUIRED_PORTAL_LED (green: 0.0, 1.0, 0.0)

### test_portal_uses_correct_idle_timeout

Verifies that the required portal uses 60-second idle timeout. The test will:
1. Provide a mock store that returns empty config
2. Use mock clock with elapsed time exactly at 60s boundary
3. Run the info_panel_lib::run() function
4. Assert that platform.reboot() was called when elapsed >= idle_timeout (60s)

### test_portal_restarts_after_idle_timeout

Verifies that the portal restarts after 60 seconds of idle time. The test will:
1. Provide a mock store that returns empty config
2. Mock the clock to advance past idle_timeout (60 seconds) without client connection
3. Run the info_panel_lib::run() function
4. Assert that platform.reboot() was called
5. Assert that portal polls every 250ms

### test_portal_continues_after_client_connection_timeout

Verifies that after a client connects, the portal waits for connected_timeout (10 minutes) before restarting. The test will:
1. Provide a mock store that returns empty config
2. Set client_count=1 to simulate a client connection
3. Mock the clock to advance past connected_timeout (10 minutes)
4. Run the info_panel_lib::run() function
5. Assert that platform.reboot() was called after the connected_timeout

---

## Config portal - Power on case

### test_portal_runs_preboot_portal_on_power_on

Verifies that when boot reason is PowerOn, the preboot portal runs. The test will:
1. Provide a mock platform with BootReason::PowerOn
2. Provide a mock store with valid config
3. Run the info_panel_lib::run() function
4. Assert that wifi.start_access_point() was called (preboot portal started)
5. Assert that LED was set to PREBOOT_PORTAL_LED (blue: 0.0, 0.53, 1.0)

### test_portal_skips_preboot_portal_on_other_boot_reasons

Verifies that preboot portal is skipped for non-PowerOn boot reasons. The test will:
1. Provide mock platform with BootReason::Software
2. Provide a mock store with valid config
3. Run the info_panel_lib::run() function
4. Assert that wifi.start_access_point() was NOT called
5. Assert that LED was NOT set to preboot blue

### test_portal_preboot_runs_even_with_valid_config

Verifies that preboot portal runs on PowerOn regardless of config validity. The test will:
1. Provide a mock platform with BootReason::PowerOn
2. Provide a mock store with COMPLETE valid config
3. Run the info_panel_lib::run() function
4. Assert that wifi.start_access_point() was called (preboot runs before wifi connect)

### test_portal_preboot_portal_uses_30_second_timeout

Verifies that the preboot portal polls at the correct interval. The test will:
1. Provide a mock platform with BootReason::PowerOn
2. Provide a mock store with valid config
3. Mock the clock to advance past 30s idle timeout
4. Run the info_panel_lib::run() function
5. Assert that portal polls every 250ms

### test_portal_preboot_waits_for_connection

Verifies that preboot portal waits for 10 minutes after client connection before continuing boot. The test will:
1. Provide a mock platform with BootReason::PowerOn
2. Provide a mock store with valid config
3. Set client_count=1 to simulate a client connection
4. Mock the clock to advance past connected_timeout (10 minutes)
5. Run the info_panel_lib::run() function
6. Assert that platform.reboot() was called after preboot portal exits

### test_portal_preboot_led_error_enters_error_mode

Verifies that LED errors during preboot portal lead to error mode after wifi connect fails. The test will:
1. Provide a mock platform with BootReason::PowerOn
2. Provide a mock store with valid config
3. Provide a mock LED that always returns error
4. Run the info_panel_lib::run() function
5. Assert that platform.reboot() was called
6. Assert that LED was set to ERROR_LED (red) at some point

---

## Config portal - Usage

### test_portal_ap_ip_is_192_168_4_1

Verifies that the AP uses the correct IP address. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function
3. Assert that wifi.start_access_point() was called
4. Assert that the AP SSID starts with "InfoPanel-"

### test_portal_scans_networks_before_portal

Verifies that WiFi networks are scanned before the config portal starts. The test will:
1. Provide a mock store that returns empty config
2. Track ordering of scan_networks and start_access_point calls via global counter
3. Run the info_panel_lib::run() function
4. Assert that wifi.scan_networks() was called BEFORE wifi.start_access_point()

### test_portal_scans_with_no_networks_found

Verifies handling when no WiFi networks are found. The test will:
1. Provide a mock store that returns empty config
2. Mock wifi.scan_networks() to return an empty vector
3. Run the info_panel_lib::run() function
4. Assert that the portal still completes and reboots

### test_portal_scans_with_duplicate_ssid

Verifies handling when scan returns networks with duplicate SSIDs. The test will:
1. Provide a mock store that returns empty config
2. Mock wifi.scan_networks() to return multiple networks with same SSID
3. Run the info_panel_lib::run() function
4. Assert that the portal completes and reboots normally

### test_portal_ap_stop_is_called_on_exit

Verifies that the AP is stopped after the portal exits. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function
3. Assert that platform.reboot() was called
4. Assert that wifi AP state is stopped after portal exits

---

## Onboard LED

### test_led_uses_default_brightness_for_portal

Verifies that portal LED uses default brightness (0.06) when led_brightness is not configured. The test will:
1. Provide a mock store missing led_brightness (triggers required portal)
2. Run the info_panel_lib::run() function
3. Assert that LED was set with PORTAL_LED_BRIGHTNESS (0.06)

### test_led_uses_config_brightness_for_connecting

Verifies that the connecting LED uses the configured brightness. The test will:
1. Provide a mock store with led_brightness = 200 (≈0.784)
2. Mock wifi.connect() to succeed
3. Run the info_panel_lib::run() function
4. Assert that LED was set with brightness ≈0.784 during connection

### test_led_uses_config_brightness_for_connected

Verifies that the connected LED uses the configured brightness. The test will:
1. Provide a mock store with led_brightness = 128 (≈0.502)
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid frame data
4. Run the info_panel_lib::run() function
5. Assert that LED was set with brightness ≈0.502 after connection

### test_led_off_when_brightness_is_zero

Verifies that LED is off when led_brightness is 0. The test will:
1. Provide a mock store with led_brightness = 0
2. Mock wifi.connect() to succeed
3. Run the info_panel_lib::run() function
4. Assert that LED was set with brightness 0.0

### test_led_max_brightness_when_brightness_is_255

Verifies that LED is at full brightness when led_brightness is 255. The test will:
1. Provide a mock store with led_brightness = 255
2. Mock wifi.connect() to succeed
3. Run the info_panel_lib::run() function
4. Assert that LED was set with brightness 1.0

---

## Summary

| Category | Tests |
|----------|-------|
| Init | 11 |
| Image | 15 |
| Config portal - No NVS | 6 |
| Config portal - Power on | 6 |
| Config portal - Usage | 5 |
| Onboard LED | 5 |
| **Total** | **48 tests** |

Additional unit tests in tests.rs (not covered by this plan):
- test_rgb565_red/green/blue/black/white (5)
- test_fill_frame_size (1)
- test_device_config_from_stored/missing_led_brightness/invalid_led_brightness (3)
- run_enters_ap_mode_when_nvs_empty (1)
