## Init

* during startup it inits and clears the TFT
* if there is value in the nvs then it starts to connect to the wifi
* during wifi connection the LED is orange
* when the wifi is connected the LED is blue

## Image

* after connecting to the wifi it makes an http request to the endpoint defined in the nvs
* if the http request returns an error it retries 3 times
* if all of the retries fail, chip enters error mode: red led for 60 seconds and then restart
* if the request is successful then it shows the image on the tft
* refresh happens every 30 seconds

## Error handling

* when there is an error then the LED is red and after 1 minute the chip is restarted

## Config portal

### No NVS case

* if the nvs is empty, it starts a wifi AP
* when the AP is running the LED is yellow
* if there is no connection for 60 seconds, it restarts
* if there is a connection then it restarts after 10 minutes

### Power on case

* if the boot reason is power on then the wifi AP is started for 30 seconds and then normal boot continues
* if there is a connection then it continues boot after 10 minutes

### Usage

* the IP address of the AP is 192.168.4.1
* the page has an html form
* when the reset button is pressed the nvs is cleared and the chip is restarted
* when the fields are filled and the save button is pressed the nvs is set and the chip is restarted
* the ssid is populated from a wifi scan

## Onboard LED

* if there is no led_brightness config in the nvs the AP and the error LED brightness is 0.5
* if there is led_brigthness in the nvs then all LEDs use that brightness value
