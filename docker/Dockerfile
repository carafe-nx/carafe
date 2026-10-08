FROM ghcr.io/autorunhq/switch-dev:2026.10.01@sha256:2f1c7e14e601cbeedb5d5235ba2ac8f7083a044dccedc8542fc157b2d7899d41

ARG LLVM_MINGW_VERSION=20260505
ARG LLVM_MINGW_SHA256_AARCH64=9fc82e4dc6598387dd3aafb642f41678f81bcd385c152a96dc74a973e7219d97
ARG LLVM_MINGW_SHA256_X86_64=986d7beef2f6d7e2f3939a1d04ac8746d614e7014557961e7e0d381d7bc5fdcd

RUN set -eu; \
    arch="$(uname -m)"; \
    case "$arch" in \
        aarch64) sha="$LLVM_MINGW_SHA256_AARCH64" ;; \
        x86_64) sha="$LLVM_MINGW_SHA256_X86_64" ;; \
        *) echo "unsupported architecture: $arch" >&2; exit 1 ;; \
    esac; \
    name="llvm-mingw-${LLVM_MINGW_VERSION}-ucrt-ubuntu-22.04-${arch}"; \
    curl -fL --retry 3 -o /tmp/llvm-mingw.tar.xz \
        "https://github.com/mstorsjo/llvm-mingw/releases/download/${LLVM_MINGW_VERSION}/${name}.tar.xz"; \
    echo "${sha}  /tmp/llvm-mingw.tar.xz" | sha256sum -c -; \
    mkdir -p /opt/llvm-mingw; \
    tar -xJf /tmp/llvm-mingw.tar.xz -C /opt/llvm-mingw --strip-components=1; \
    rm /tmp/llvm-mingw.tar.xz

ENV PATH="/opt/llvm-mingw/bin:${PATH}"
