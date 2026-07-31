# syntax=docker/dockerfile:1

FROM node:24-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
# --ignore-scripts : le postinstall (nuxt prepare) a besoin des sources,
# copiées à l'étape suivante ; nuxt build refait le prepare de toute façon.
RUN npm ci --ignore-scripts
COPY . .
RUN npm run build

FROM node:24-slim AS runtime
ENV NODE_ENV=production \
    NITRO_HOST=0.0.0.0 \
    NITRO_PORT=3000
WORKDIR /app
COPY --from=build --chown=node:node /app/.output ./.output
# Migrations exécutables depuis cette image (job dédié au déploiement,
# PRD §7.3) : node .output/server/migrate.mjs — le script est placé dans
# .output/server/ pour résoudre pg/drizzle-orm depuis ses node_modules.
COPY --from=build --chown=node:node /app/server/database/migrations ./.output/server/migrations
COPY --from=build --chown=node:node /app/scripts/migrate.mjs ./.output/server/migrate.mjs
USER node
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+(process.env.NITRO_PORT||process.env.PORT||3000)+'/api/health').then(r => process.exit(r.ok ? 0 : 1)).catch(() => process.exit(1))"
CMD ["node", ".output/server/index.mjs"]
