# Docker smoke fixture: café λ
FROM alpine:3.20
WORKDIR /app
COPY . .
RUN echo "ok"
CMD ["echo", "hello"]
