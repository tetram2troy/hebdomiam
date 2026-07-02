import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

// Tests d'intégration : DB PostgreSQL éphémère (hebdomiam_test), créée et migrée
// par le global setup. Nécessite la DB de dev démarrée
// (docker compose -f docker-compose.dev.yml up -d) ou le service Postgres de la CI.
export default defineConfig({
  resolve: {
    alias: {
      '#shared': fileURLToPath(new URL('./shared', import.meta.url)),
    },
  },
  test: {
    include: ['tests/integration/**/*.test.ts'],
    environment: 'node',
    globalSetup: ['tests/integration/global-setup.ts'],
    // Les migrations + transactions réelles prennent plus de temps que l'unitaire.
    testTimeout: 15000,
    hookTimeout: 30000,
    // Une seule instance : les tests partagent la même base et se tronquent entre eux.
    fileParallelism: false,
  },
})
