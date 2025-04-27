#!/bin/bash
stty -F /dev/ttyUSB0 2400
stty -F /dev/ttyUSB0 raw

while true
do

	echo "1"

	echo -ne "QPIGS2\x68\x2d\x0d" > /dev/ttyUSB0
	sleep 1

	echo "2"
	echo -ne "QPIGS\xb7\xa9\x0d" > /dev/ttyUSB0
	sleep 1

	echo "3"
	echo -ne "QPIWS\xb4\xda\x0d" > /dev/ttyUSB0
	sleep 1

done
