#!/bin/bash

bauds=( 1200 1800 2400 4800 9600 19200 28800 38400 57600 76800 115200 )
messages=( 4E5700130000000006030000000000006800000129 AA5590EB96000000000000000000000000000010 AA5590EB97000000000000000000000000000011 )

for baud in "${bauds[@]}"
do
   : 
		for message in "${messages[@]}"
		do
			 : 
						node discover.ts --baud "$baud" --message "$message"
		done
done
