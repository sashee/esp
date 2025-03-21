FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y \
# rustup
	curl \
# espflash
	gcc build-essential curl pkg-config \
# extras
	libssl-dev libudev-dev \
# https://docs.espressif.com/projects/esp-idf/en/latest/esp32/get-started/linux-macos-setup.html#for-linux-users
	git wget flex bison gperf python3 python3-pip python3-venv cmake ninja-build ccache libffi-dev libssl-dev dfu-util libusb-1.0-0 \
# llvm
	llvm \
# fish
	fish

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN rustup toolchain install stable --component rust-src 
# For std applications, you need to use nightly (https://docs.esp-rs.org/book/installation/riscv.html)
RUN rustup toolchain install nightly --component rust-src

RUN cargo install espflash
RUN cargo install cargo-generate
RUN cargo install esp-generate

RUN rustup target add riscv32imac-unknown-none-elf
RUN cargo install ldproxy

WORKDIR /opt/esp/project

ENTRYPOINT ["/bin/fish"]
