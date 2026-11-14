# Build image for the Linux release binaries of mdedit: an old glibc (Ubuntu 20.04, glibc 2.31)
# so the AppImage runs on every distribution at least that new. Used by build-appimage.sh.
FROM ubuntu:20.04

ARG RUST_TOOLCHAIN=stable
ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:$PATH

# Link-time needs are libc, fontconfig, freetype and X11/xcb; wayland, libcurl and the like are
# loaded at run time and need no -dev package. git: cargo fetches the shared crates from GitHub.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl git pkg-config cmake \
        libfontconfig1-dev libfreetype6-dev \
        libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
        libxkbcommon-dev libxkbcommon-x11-dev libx11-xcb-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain ${RUST_TOOLCHAIN} \
    && chmod -R a+rX /opt/rustup /opt/cargo

# Packaging tools: rpmbuild (make-rpm.sh), lintian (make-deb.sh runs it when present); dpkg-deb comes
# with dpkg. Their own layer, so the layers above stay cached.
RUN apt-get update && apt-get install -y --no-install-recommends rpm lintian xz-utils \
    && rm -rf /var/lib/apt/lists/*

# The build runs as the caller's user; the registry and the target directory are mounted at /cache.
ENV HOME=/tmp/home
WORKDIR /src
