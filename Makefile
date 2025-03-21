ROOT_DIR := $(dir $(realpath $(lastword $(MAKEFILE_LIST))))

dev:
	docker build -t esp .
	docker run --rm -it -v $(ROOT_DIR):/opt/esp --device=/dev/ttyACM0 esp

.PHONY: dev
