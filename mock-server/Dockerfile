# The demo API: the stateful mock on Bun. It binds to loopback by design; the web container shares this
# container's network namespace and is the only way in (see docker-compose.yml).
FROM oven/bun:1.4.2-alpine

WORKDIR /app/mock-server
COPY mock-server/package.json mock-server/bun.lock ./
RUN bun install --frozen-lockfile --production
COPY mock-server/src ./src
# The catalog, knowledge banks and golden GitHub fixtures are shared with the Rust domain.
COPY backend/crates/domain/data /app/backend/crates/domain/data
COPY contracts/fixtures /app/contracts/fixtures
COPY mock-server/fixtures/DejaVuSans.ttf mock-server/fixtures/DejaVu-LICENSE.txt ./fixtures/
# File modes follow the checkout's umask; the service user only needs to read.
RUN chmod -R a+rX /app

ENV NODE_ENV=production \
    HOST=127.0.0.1 \
    PORT=4010 \
    FSP_MOCK_SCENARIO=baseline \
    FSP_MOCK_DEV_ROUTES=0 \
    BUN_RUNTIME_TRANSPILER_CACHE_PATH=0
USER bun
CMD ["bun", "src/server.ts"]
