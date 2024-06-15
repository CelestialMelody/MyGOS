# syntax=docker/dockerfile:1
# - Extensive comments linking to relevant documentation
FROM ubuntu:20.04

ARG QEMU_VERSION=7.0.0
ARG HOME=/root

# 0. Install general tools
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && \
    apt-get install -y \
    curl \
    git \
    python3 \
    wget

# 1. Set up QEMU RISC-V
# - https://learningos.github.io/rust-based-os-comp2022/0setup-devel-env.html#qemu
# - https://www.qemu.org/download/
# - https://wiki.qemu.org/Documentation/Platforms/RISCV
# - https://risc-v-getting-started-guide.readthedocs.io/en/latest/linux-qemu.html

# 1.1. Download source
WORKDIR ${HOME}
RUN wget https://download.qemu.org/qemu-${QEMU_VERSION}.tar.xz && \
    tar xvJf qemu-${QEMU_VERSION}.tar.xz

# 1.2. Install dependencies
# - https://risc-v-getting-started-guide.readthedocs.io/en/latest/linux-qemu.html#prerequisites
RUN apt-get install -y \
    autoconf automake autotools-dev curl libmpc-dev libmpfr-dev libgmp-dev \
    gawk build-essential bison flex texinfo gperf libtool patchutils bc \
    zlib1g-dev libexpat-dev git \
    ninja-build pkg-config libglib2.0-dev libpixman-1-dev

# 1.3. Build and install from source
WORKDIR ${HOME}/qemu-${QEMU_VERSION}
RUN ./configure --target-list=riscv64-softmmu,riscv64-linux-user && \
    make -j$(nproc) && \
    make install

# 1.4. Clean up
WORKDIR ${HOME}
RUN rm -rf qemu-${QEMU_VERSION} qemu-${QEMU_VERSION}.tar.xz

# 1.5. Sanity checking
RUN qemu-system-riscv64 --version && \
    qemu-riscv64 --version

# 2. Set up Rust
# - https://learningos.github.io/rust-based-os-comp2022/0setup-devel-env.html#qemu
# - https://www.rust-lang.org/tools/install
# - https://github.com/rust-lang/docker-rust/blob/master/Dockerfile-debian.template

# 2.1. Install
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=nightly
RUN set -eux; \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup-init; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile minimal --default-toolchain $RUST_VERSION; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME;

# 2.2. Sanity checking
RUN rustup --version && \
    cargo --version && \
    rustc --version

# 3. Build env for labs
# See os1/Makefile `env:` for example.
# This avoids having to wait for these steps each time using a new container.
# so when you have build the image, you can remove(delete) os/Makfile `env:` steps
RUN (rustup target list | grep "riscv64gc-unknown-none-elf (installed)") || rustup target add riscv64gc-unknown-none-elf && \
    rustup target add riscv64gc-unknown-none-elf && \
    cargo install cargo-binutils --vers ~0.3 && \
    rustup component add rust-src && \
    rustup component add llvm-tools-preview

# 4. Build riscv gnu toolchain
WORKDIR ${HOME}
RUN git clone --recursive https://github.com/riscv/riscv-gnu-toolchain

WORKDIR ${HOME}/riscv-gnu-toolchain
ENV RISCV_GNU_TOOLCHAIN_HOME=/usr/local/riscv-gnu-toolchain \
    PATH=/usr/local/riscv-gnu-toolchain/bin:$PATH
RUN    ./configure --prefix=/usr/local/riscv-gnu-toolchain && make -j$(nproc)
# RUN export PATH="$PATH:/usr/local/riscv-gnu-toolchain/bin" # add to .bashrc

# 5. debug tools
RUN apt-get update && \
    apt-get install -y \
    python3-pip \
    vim \
    tmux
# RUN pip3 install pygments

# 6. add tools
# [1] use dtc to get device tree
RUN apt-get install device-tree-compiler -y

# Ready to go
WORKDIR ${HOME}