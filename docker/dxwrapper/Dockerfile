FROM debian:trixie@sha256:2dc419c6d7fe3dad0a2f9fd469c34cc90b0f9ba642d8fde837fe3d7d34a0e7f1

ARG MSVC_WINE_REVISION=514f8ea34842cd6d831804d0e9658d3a32870ae1

RUN apt-get update && \
    apt-get install -y --no-install-recommends wine wine64 python3 msitools ca-certificates git procps && \
    apt-get clean -y && \
    rm -rf /var/lib/apt/lists/*

RUN WINEDLLOVERRIDES="mscoree,mshtml=" wine wineboot --init && \
    while pgrep wineserver > /dev/null; do sleep 1; done

RUN git clone -q https://github.com/mstorsjo/msvc-wine.git /tmp/msvc-wine && \
    git -C /tmp/msvc-wine checkout -q "$MSVC_WINE_REVISION" && \
    cd /tmp/msvc-wine && \
    PYTHONUNBUFFERED=1 ./vsdownload.py --accept-license --major 17 --msvc-version 17.14 --sdk-version 10.0.26100 \
        --architecture x86 --dest /opt/msvc && \
    ./install.sh /opt/msvc && \
    cp msvcenv-native.sh /opt/msvc/ && \
    rm -rf /tmp/msvc-wine

ENV PATH="/opt/msvc/bin/x86:${PATH}"
