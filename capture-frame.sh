#!/bin/bash

DATE=$(date -Iseconds)

ffmpeg -i /dev/video0 -frames 1 -vf "drawtext=fontfile=/usr/share/fonts/truetype/noto/NotoMono-Regular.ttf: text='%{localtime}': x=(w-tw)/2: y=h-(2*lh): fontcolor=white: box=1: boxcolor=0x00000000@1" -y "/home/pi/webcam2/out-$DATE.jpg"

convert "/home/pi/webcam2/out-$DATE.jpg" -distort Perspective '319,364,0,0 263,300,300,0 318,282,300,300 378,342,0,300' -crop 300x300+0+0 - | convert - -gravity NorthWest -pointsize 12 -fill white -annotate +0+0 "$DATE" "/home/pi/webcam2/fixed/out-$DATE.jpg"
