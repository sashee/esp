# Info Panel Lib Test Plan

## Init

### test_init_clears_tft_on_startup

Verifies that during startup, the display is initialized BEFORE any WiFi connection. The test will:
1. Provide a mock store with valid config (ssid, pw, url, led_brightness)
2. Track the order of display.init() vs wifi.connect() calls
3. Mock wifi.connect() to succeed
4. Mock http_client.get() to return valid frame data
5. Run the info_panel_lib::run() function
6. Assert that display.init() was called BEFORE wifi.connect() is called

### test_init_returns_error_when_display_init_fails

Verifies that if display.init() fails, the error propagates correctly. The test will:
1. Provide a mock display that returns an error on init()
2. Run the info_panel_lib::run() function
3. Assert that the returned Result contains the display init error

### test_init_connects_wifi_when_nvs_has_complete_config

Verifies that when NVS has complete config (ssid, pw, url, led_brightness), WiFi connection is attempted. The test will:
1. Provide a mock store with all required config values
2. Mock wifi.connect() to return success
3. Mock http_client.get() to return valid frame data
4. Run the info_panel_lib::run() function
5. Assert that wifi.connect() was called with the stored ssid and password
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
4. Assert that error message indicates "configuration missing"

### test_init_goes_to_required_portal_when_nvs_empty

Verifies that when NVS is completely empty, the required config portal is started. The test will:
1. Provide a mock store that returns an empty BTreeMap
2. Run the info_panel_lib::run() function
3. Assert that wifi.start_access_point() was called
4. Assert that LED was set to REQUIRED_PORTAL_LED (green)

### test_init_sets_orange_led_during_wifi_connection

Verifies that the LED is orange during WiFi connection. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to use a callback that receives connect states
3. Run the info_panel_lib::run() function
4. Assert that at some point during connection, LED was set to CONNECTING_LED (1.0, 0.78, 0.0) with brightness from config.led_brightness()

### test_init_continues_when_led_set_fails_during_connect

Verifies that LED errors during connection don't stop the flow. The test will:
1. Provide a mock LED that returns an error on set_pixel
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid frame data
4. Run the info_panel_lib::run() function
5. Assert that the function completes successfully despite LED errors

### test_init_sets_blue_led_when_wifi_connected

Verifies that the LED is blue when WiFi is successfully connected. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to return success
3. Mock http_client.get() to return valid frame data
4. Run the info_panel_lib::run() function
5. Assert that LED was set to CONNECTED_LED (0.0, 0.0, 1.0) with config.led_brightness()

---

## Image

### test_image_fetches_url_after_wifi_connect

Verifies that after WiFi connects, the HTTP request is made to the configured URL. The test will:
1. Provide a mock store with valid config and url = "http://example.com/frame.bin"
2. Mock wifi.connect() to succeed
3. Track what URL was passed to http_client.get()
4. Run the info_panel_lib::run() function
5. Assert that http_client.get() was called with "http://example.com/frame.bin"

### test_image_fetches_empty_url_when_url_empty

Verifies behavior when the configured URL is an empty string. The test will:
1. Provide a mock store with url = ""
2. Mock wifi.connect() to succeed
3. Run the info_panel_lib::run() function
4. Assert that http_client.get() was still called (even with empty URL)

### test_image_fails_when_url_invalid

Verifies that an invalid URL causes HTTP failure. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return an error (e.g., "invalid url")
4. Run the info_panel_lib::run() function
5. Assert that the error propagates to the retry logic

### test_image_retries_3_times_on_http_failure

Verifies that when HTTP fails, exactly 3 retry attempts are made. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to always return an error
4. Track the number of times http_client.get() was called
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() was called exactly 3 times

### test_image_succeeds_on_first_retry

Verifies that if the first retry succeeds, no more retries are made. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to fail on first call, succeed on second
4. Track the number of times http_client.get() was called
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() was called exactly 2 times (1 fail + 1 success)

### test_image_succeeds_on_second_retry

Verifies that if the second retry succeeds, no third retry is made. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to fail on first two calls, succeed on third
4. Track the number of times http_client.get() was called
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() was called exactly 3 times

### test_image_succeeds_on_third_retry

Verifies that the third retry succeeding completes the flow. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to fail twice, succeed on third call
4. Run the info_panel_lib::run() function
5. Assert that display.write_frame() was called

### test_image_uses_last_error_when_all_retries_fail

Verifies that when all retries fail, the last error is used. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to fail with different errors each time
4. Run the info_panel_lib::run() function
5. Assert that the final error matches the last error from http_client.get()

### test_image_enters_error_mode_after_all_retries_fail

Verifies that after all 3 retries fail, the device enters error mode with red LED and reboot. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to always fail
4. Run the info_panel_lib::run() function (expecting panic from reboot)
5. Assert that LED was set to ERROR_LED (red: 1.0, 0.0, 0.0) before reboot
6. Assert that platform.reboot() was called

### test_image_error_mode_waits_before_restart

Verifies that error mode waits for the configured delay before restarting. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to always fail
4. Track when LED is set to ERROR_LED vs when reboot is called
5. Run the info_panel_lib::run() function
6. Assert that the time between ERROR_LED and reboot matches RUNTIME_ERROR_REBOOT_DELAY (10 minutes)

### test_image_displays_frame_on_tft_when_fetch_succeeds

Verifies that when HTTP fetch succeeds, the frame is written to the TFT display. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid frame bytes (128x160x2 bytes)
4. Run the info_panel_lib::run() function
5. Assert that display.write_frame() was called with the returned bytes

### test_image_handles_write_frame_failure

Verifies that write_frame failures in the main loop are handled gracefully. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid frame data
4. Mock display.write_frame() to fail on first call, succeed on subsequent
5. Run the info_panel_lib::run() function
6. Assert that the loop continues after write_frame error

### test_image_handles_invalid_frame_size

Verifies that frames of unexpected size are still passed to write_frame. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return frame of wrong size (e.g., 100 bytes instead of 40960)
4. Run the info_panel_lib::run() function
5. Assert that display.write_frame() was called with the invalid-size data (no validation in library)

### test_image_refreshes_after_30_second_interval

Verifies that the image refreshes every 30 seconds. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to return valid data
4. Track the number of http_client.get() calls over multiple loop iterations
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() is called again after Clock::sleep(Duration::from_secs(30))

### test_image_aborts_refresh_on_wifi_disconnect

Verifies that if WiFi disconnects during the 30-second wait, an error is returned. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed initially
3. Mock wifi.is_connected() to return false on second check
4. Run the info_panel_lib::run() function
5. Assert that the function returns an error with "wifi disconnected"

### test_image_continues_refresh_loop_after_write_frame_error

Verifies that write_frame errors in the refresh loop don't stop the refresh cycle. The test will:
1. Provide a mock store with valid config
2. Mock wifi.connect() to succeed
3. Mock http_client.get() to always return valid data
4. Mock display.write_frame() to fail
5. Run the info_panel_lib::run() function
6. Assert that http_client.get() is called multiple times (loop continues)

---

## Config portal - No NVS case

### test_portal_starts_ap_when_nvs_empty

Verifies that when NVS is empty, an access point is started. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function
3. Assert that wifi.start_access_point() was called
4. Assert that the AP SSID starts with "InfoPanel-"

### test_portal_handles_ap_start_failure

Verifies that AP start failures are handled. The test will:
1. Provide a mock store that returns empty config
2. Mock wifi.start_access_point() to return an error
3. Run the info_panel_lib::run() function
4. Assert that the error propagates (no panic)

### test_portal_sets_green_led_when_required_portal_runs

Verifies that the LED is green when the required config portal runs. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function
3. Assert that LED was set to REQUIRED_PORTAL_LED (green: 0.0, 1.0, 0.0)

### test_portal_led_turned_off_after_portal_exits

Verifies that the LED is turned off after the required portal exits. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function (expecting it to complete after portal)
3. Assert that LED was set to OFF_LED (0.0, 0.0, 0.0) after portal exits

### test_portal_restarts_after_idle_timeout

Verifies that the portal restarts after 60 seconds of idle time. The test will:
1. Provide a mock store that returns empty config
2. Mock the clock to advance past idle_timeout (60 seconds) without client connection
3. Run the info_panel_lib::run() function
4. Assert that platform.reboot() was called

### test_portal_uses_correct_idle_timeout

Verifies that the required portal uses 60-second idle timeout. The test will:
1. Provide a mock store that returns empty config
2. Track timing between portal start and reboot
3. Run the info_panel_lib::run() function
4. Assert that reboot happens approximately 60 seconds after portal starts

### test_portal_continues_after_client_connection_timeout

Verifies that after a client connects, the portal waits for connected_timeout (10 minutes) before exiting. The test will:
1. Provide a mock store that returns empty config
2. Simulate a client connection event
3. Mock the clock to advance past connected_timeout (10 minutes)
4. Run the info_panel_lib::run() function
5. Assert that portal exits after timeout and function completes

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
1. Provide mock platforms with various BootReasons (Software, Watchdog, Panic, etc.)
2. Provide a mock store with valid config
3. For each boot reason, run the info_panel_lib::run() function
4. Assert that wifi.start_access_point() was NOT called for any except PowerOn

### test_portal_preboot_runs_even_with_valid_config

Verifies that preboot portal runs on PowerOn regardless of config validity. The test will:
1. Provide a mock platform with BootReason::PowerOn
2. Provide a mock store with COMPLETE valid config
3. Run the info_panel_lib::run() function
4. Assert that wifi.start_access_point() was called (preboot runs before wifi connect)

### test_portal_preboot_portal_uses_30_second_timeout

Verifies that the preboot portal uses 30-second idle timeout. The test will:
1. Provide a mock platform with BootReason::PowerOn
2. Provide a mock store with valid config
3. Track timing of portal start
4. Run the info_panel_lib::run() function
5. Assert that reboot/idle happens approximately 30 seconds after portal starts (PREBOOT_PORTAL_TIMING)

### test_portal_preboot_waits_for_connection

Verifies that preboot portal waits for 10 minutes after client connection. The test will:
1. Provide a mock platform with BootReason::PowerOn
2. Provide a mock store with valid config
3. Simulate client connection to the preboot AP
4. Mock the clock to advance past connected_timeout (10 minutes)
5. Run the info_panel_lib::run() function
6. Assert that portal exits and continues to normal boot flow after timeout

---

## Config portal - Usage

### test_portal_ap_ip_is_192_168_4_1

Verifies that the AP uses the correct IP address. The test will:
1. Provide a mock store that returns empty config
2. Run the info_panel_lib::run() function
3. Assert that the IP config returned by access_point_ip_config() is 192.168.4.1

### test_portal_scans_networks_before_portal

Verifies that WiFi networks are scanned before the config portal starts. The test will:
1. Provide a mock store that returns empty config
2. Mock wifi.scan_networks() to return some test networks
3. Run the info_panel_lib::run() function
4. Assert that wifi.scan_networks() was called before wifi.start_access_point()

### test_portal_scans_with_no_networks_found

Verifies handling when no WiFi networks are found. The test will:
1. Provide a mock store that returns empty config
2. Mock wifi.scan_networks() to return an empty vector
3. Run the info_panel_lib::run() function
4. Assert that the portal still starts (SSID dropdown will be empty)

### test_portal_scans_with_duplicate_ssid

Verifies handling when scan returns networks with duplicate SSIDs. The test will:
1. Provide a mock store that returns empty config
2. Mock wifi.scan_networks() to return multiple networks with same SSID
3. Run the info_panel_lib::run() function
4. Assert that the portal handles duplicates (no duplicates in dropdown - deduped by config_portal)

---

## Onboard LED

### test_led_uses_default_brightness_for_portal

Verifies that portal LED uses default brightness when led_brightness is not configured. The test will:
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
| Init | 9 |
| Image | 13 |
| Config portal - No NVS | 6 |
| Config portal - Power on | 5 |
| Config portal - Usage | 3 |
| Onboard LED | 5 |
| **Total** | **41 tests** |
