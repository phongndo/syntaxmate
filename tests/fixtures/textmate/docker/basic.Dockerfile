# syntax=docker/dockerfile:1.7
# Small café service for λ users: 🚀 𝌆
ARG ALPINE_VERSION=3.20
FROM alpine:${ALPINE_VERSION}

LABEL org.opencontainers.image.title="Café API" \
      org.opencontainers.image.description='Unicode-ready λ service 🚀' \
      org.opencontainers.image.vendor="Example 𝌆 Labs"
ENV APP_HOME=/srv/cafe \
    APP_MODE=production
WORKDIR ${APP_HOME}

RUN addgroup -S app \
 && adduser -S -D -H -G app app
COPY --chown=app:app app/ ./
RUN <<'WELCOME'
set -eu
printf '%s\n' 'café λ 🚀 𝌆' > welcome.txt
WELCOME

USER app
EXPOSE 8080
HEALTHCHECK --interval=30s CMD ["wget", "-qO-", "http://127.0.0.1:8080/health"]
ENTRYPOINT ["./server"]
CMD ["--port", "8080"]
