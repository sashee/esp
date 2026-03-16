## Init

* during startup it inits and clears the TFT

## Image

## Error handling

## Config portal

* if the nvs is empty, it starts a wifi AP
* when the AP is running the LED is yellow
* if there is no connection for 60 seconds, it restarts
* if there is a connection then it restarts after 10 minutes
* the IP address of the AP is 192.168.4.1
* the page has an html form
