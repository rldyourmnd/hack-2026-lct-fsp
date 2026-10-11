# The web application: a static Vite build served by unprivileged nginx, which also proxies /api/v1 to the API
# on 127.0.0.1:4010 in the shared network namespace (see docker-compose.yml).
FROM oven/bun:1.4.2-slim AS build

WORKDIR /app/frontend
COPY frontend/package.json frontend/bun.lock ./
RUN bun install --frozen-lockfile
COPY frontend/ ./
# The public demo keeps three seeded accounts available while all other addresses use real email.
ARG VITE_DEMO_STAND=1
ARG VITE_DEMO_EMAIL_CODE=123456
RUN bun run build && bun scripts/check-bundle.ts

FROM nginxinc/nginx-unprivileged:1.30.5-alpine
# File modes follow the checkout's umask; nginx runs unprivileged and only needs to read.
COPY --chmod=0644 infra/docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --chmod=0644 infra/nginx/fsp-security-headers.conf /etc/nginx/fsp-security-headers.conf
COPY --from=build /app/frontend/dist /usr/share/nginx/html
