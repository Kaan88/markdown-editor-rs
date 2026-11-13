# Build image for cross-compiling the Windows (x86_64-pc-windows-msvc) release files of mdedit on
# Linux: rustup stable, the MSVC target, cargo-xwin and LLVM (clang, lld, llvm-rc).
#
# cargo-xwin downloads the MSVC C runtime and the Windows SDK from Microsoft at its first run;
# using them means accepting Microsoft's licence (https://go.microsoft.com/fwlink/?LinkId=2086102).
# cargo-xwin 0.23.1 has no accept flag; it accepts the licence itself when it downloads, so a build
# with this image is that acceptance (see release-cross.sh).
FROM ubuntu:24.04

ARG RUST_TOOLCHAIN=stable
ARG CARGO_XWIN_VERSION=0.23.1
ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl git pkg-config libssl-dev cmake \
        clang lld llvm \
    && rm -rf /var/lib/apt/lists/* \
    # Unversioned names for the LLVM tools the build looks up (lld-link, llvm-rc, llvm-lib ...).
    && for t in lld-link llvm-rc llvm-lib llvm-ar llvm-dlltool llvm-readobj llvm-objdump; do \
         if ! command -v $t >/dev/null; then \
           p=$(ls /usr/bin/$t-* 2>/dev/null | sort -V | tail -1); [ -n "$p" ] && ln -s "$p" /usr/local/bin/$t; \
         fi; \
       done

RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal \
        --default-toolchain ${RUST_TOOLCHAIN} --target x86_64-pc-windows-msvc \
    && cargo install --locked cargo-xwin --version ${CARGO_XWIN_VERSION} \
    && rm -rf /opt/cargo/registry /opt/cargo/git \
    && chmod -R a+rX /opt/rustup /opt/cargo

# The build runs as the caller's user; the registry, target directory and the xwin SDK cache are
# mounted under /cache.
ENV HOME=/tmp/home
WORKDIR /src
