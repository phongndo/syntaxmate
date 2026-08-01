# syntax=docker/dockerfile:1.7
# escape=\
# check=error=true
# Atlas service image: café, λ, 日本語, and launch 🚀

ARG GO_VERSION=1.23.4
ARG NODE_VERSION=22.12.0
ARG ALPINE_VERSION=3.21
ARG BUILDPLATFORM=linux/amd64
ARG VERSION=0.0.0-dev
ARG REVISION=unknown

# Shared toolchain for the API and web asset stages.
FROM --platform=${BUILDPLATFORM} golang:${GO_VERSION}-alpine${ALPINE_VERSION} AS toolchain
MAINTAINER Atlas Build Team <builds@example.test>
ARG TARGETOS
ARG TARGETARCH
ENV CGO_ENABLED=0 \
    GOFLAGS="-mod=readonly -trimpath" \
    GOTOOLCHAIN=local
WORKDIR /workspace
RUN apk add --no-cache \
      ca-certificates \
      git \
      tzdata
RUN --mount=type=cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,target=/root/.cache/go-build,sharing=locked \
    go env -w GOPROXY="https://proxy.golang.org,direct"
SHELL ["/bin/ash", "-eo", "pipefail", "-c"]

# Resolve Go modules separately so source edits retain the cache layer.
FROM toolchain AS go-deps
COPY ["go.mod", "go.sum", "./"]
RUN --mount=type=cache,target=/go/pkg/mod,sharing=locked go mod download
RUN ["go", "version"]

FROM go-deps AS api-build
COPY cmd/ ./cmd/
COPY internal/ ./internal/
COPY pkg/ ./pkg/
ADD --chown=0:0 configs/default.yaml ./configs/default.yaml
ARG VERSION
ARG REVISION
RUN --mount=type=cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,target=/root/.cache/go-build,sharing=locked \
    GOOS=${TARGETOS:-linux} GOARCH=${TARGETARCH:-amd64} \
    go build -ldflags "-s -w -X main.version=${VERSION} -X main.revision=${REVISION}" \
      -o /out/atlas ./cmd/atlas
RUN test -x /out/atlas && /out/atlas version

# Node dependency stage uses JSON and shell instruction forms.
FROM --platform=$BUILDPLATFORM node:${NODE_VERSION}-alpine${ALPINE_VERSION} AS web-deps
ARG NPM_REGISTRY=https://registry.npmjs.org/
ENV NODE_ENV=development
WORKDIR /workspace/web
COPY ["web/package.json", "web/package-lock.json", "./"]
RUN --mount=type=cache,target=/root/.npm,sharing=locked \
    npm ci --registry="$NPM_REGISTRY" --ignore-scripts
RUN ["node", "--version"]

FROM web-deps AS web-build
COPY --link web/ ./
ARG PUBLIC_PATH=/assets/
ARG VERSION
ENV VITE_PUBLIC_PATH=${PUBLIC_PATH} \
    VITE_RELEASE="$VERSION"
RUN --network=none npm run build
RUN test -f dist/index.html

# Exercise a quoted heredoc while keeping every Docker string balanced.
FROM alpine:${ALPINE_VERSION} AS generated-config
WORKDIR /generated
RUN <<'GENERATE_CONFIG'
set -eu
mkdir -p /generated
printf '%s\n' 'service_name: atlas' > /generated/runtime.yaml
printf '%s\n' 'welcome: café λ 日本語 🚀' >> /generated/runtime.yaml
printf '%s\n' 'literal_variable: ${NOT_EXPANDED}' >> /generated/runtime.yaml
GENERATE_CONFIG
RUN cat /generated/runtime.yaml

# Unit tests deliberately retain a shell-form default command.
FROM api-build AS api-test
ENV TEST_COLOR="always" TEST_TIMEOUT=45s
COPY --from=web-build /workspace/web/dist ./web/dist
RUN --mount=type=cache,target=/root/.cache/go-build,sharing=locked \
    go test ./... -count=1 -timeout "$TEST_TIMEOUT"
HEALTHCHECK NONE
CMD go test ./... -count=1

FROM web-build AS web-test
ENV CI=true
RUN npm test -- --runInBand
ENTRYPOINT npm test --
CMD --runInBand

# Fetch a pinned public artifact in an isolated verification stage.
FROM alpine:${ALPINE_VERSION} AS legal-assets
ARG LICENSE_SHA256=3b7c3f7a5f14e2f0f2c5d902f932b31ca2e31ad1d31f7f36c321833a09ce077b
ADD --checksum=sha256:${LICENSE_SHA256} \
    https://example.test/artifacts/atlas-license.txt \
    /licenses/atlas-license.txt
RUN test -s /licenses/atlas-license.txt

# A reusable base demonstrates metadata, legacy ENV, and signal syntax.
FROM alpine:${ALPINE_VERSION} AS runtime-base
ARG VERSION
ARG REVISION
LABEL org.opencontainers.image.title="Atlas Service" \
      org.opencontainers.image.description="Small API with a café-ready web UI" \
      org.opencontainers.image.vendor='Example Systems' \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision=$REVISION \
      com.example.note="Atlas \"Rocket\" image 🚀"
ENV APP_HOME /srv/atlas
ENV PATH="${APP_HOME}/bin:${PATH}" \
    LANG=C.UTF-8 \
    APP_MODE=production
RUN addgroup -S -g 10001 atlas \
 && adduser -S -D -H -u 10001 -G atlas atlas \
 && mkdir -p "$APP_HOME/bin" "$APP_HOME/web" /var/lib/atlas /run/atlas \
 && chown -R atlas:atlas "$APP_HOME" /var/lib/atlas /run/atlas
WORKDIR ${APP_HOME}
STOPSIGNAL SIGTERM
EXPOSE 8080 9090/tcp
VOLUME ["/var/lib/atlas"]

# Production image copies only verified outputs from named stages.
FROM runtime-base AS production
ARG VERSION
ARG REVISION
COPY --from=api-build --chown=atlas:atlas --chmod=0755 /out/atlas ./bin/atlas
COPY --from=web-build --chown=atlas:atlas /workspace/web/dist/ ./web/
COPY --from=generated-config --chown=atlas:atlas /generated/runtime.yaml ./config/runtime.yaml
COPY --from=legal-assets /licenses/atlas-license.txt /licenses/atlas-license.txt
USER 10001:10001
ENV HTTP_PORT=8080 METRICS_PORT=9090
EXPOSE ${HTTP_PORT}/tcp ${METRICS_PORT}
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD ["/srv/atlas/bin/atlas", "healthcheck", "--url=http://127.0.0.1:8080/ready"]
ENTRYPOINT ["/srv/atlas/bin/atlas"]
CMD ["serve", "--config", "/srv/atlas/config/runtime.yaml"]

# Development target keeps tooling and writable source mounts available.
FROM toolchain AS development
ARG DEV_UID=1000
ARG DEV_GID=1000
ENV HOME=/home/developer \
    APP_ENV='development'
RUN addgroup -g "$DEV_GID" developer \
 && adduser -D -u "$DEV_UID" -G developer developer
WORKDIR /workspace
COPY --chown=developer:developer . .
USER developer:developer
VOLUME /workspace/tmp /workspace/.cache
EXPOSE 3000
SHELL ["/bin/ash", "-lc"]
CMD ["go", "run", "./cmd/atlas", "serve"]

# Downstream image template covers deferred ONBUILD instructions.
FROM runtime-base AS downstream-template
ONBUILD ARG DOWNSTREAM_VERSION=dev
ONBUILD ENV ATLAS_DOWNSTREAM=true
ONBUILD LABEL com.example.downstream="${DOWNSTREAM_VERSION}"
ONBUILD WORKDIR /srv/atlas
ONBUILD COPY --chown=atlas:atlas ./extensions/ ./extensions/
ONBUILD RUN test -d ./extensions
ONBUILD USER atlas
ONBUILD ENTRYPOINT ["/srv/atlas/bin/atlas"]
ONBUILD CMD ["serve", "--config", "/srv/atlas/config/runtime.yaml"]
STOPSIGNAL 15

# Minimal diagnostic target also confirms case-insensitive instructions.
from alpine:${ALPINE_VERSION} AS diagnostics
ADD ["scripts/diagnose.sh", "/usr/local/bin/diagnose"]
run chmod 0755 /usr/local/bin/diagnose \
 && printf "quote=\"ok\" backslash=\\\\ newline=\\n\n" > /tmp/escapes.txt
USER nobody
ENTRYPOINT ["/usr/local/bin/diagnose"]
CMD --verbose
