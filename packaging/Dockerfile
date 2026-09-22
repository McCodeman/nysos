# SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
# SPDX-License-Identifier: Apache-2.0

FROM rust:1.94.0-alpine3.23@sha256:ff0adc35894eb79586ce752a1b5a9eadc88b938c56d8f2b4b537b6258ff3fa10 AS build
RUN apk add --no-cache build-base
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY examples ./examples
ARG NYSOS_GIT_COMMIT=unknown
ARG NYSOS_GIT_TAG=unknown
ARG NYSOS_GIT_DESCRIBE=unknown
ARG SOURCE_DATE_EPOCH
ARG TARGETARCH
ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target,id=nysos-target-${TARGETARCH} \
    target=$(rustc -vV | sed -n 's/^host: //p') && \
    cargo build --locked --release --bin nysos --target "$target" && \
    mkdir /out && cp "target/$target/release/nysos" /out/nysos && \
    readelf -l /out/nysos > /out/elf.txt && ! grep -q INTERP /out/elf.txt
FROM scratch
COPY --from=build /out/nysos /nysos
