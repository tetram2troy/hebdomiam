import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  // Seed du compte admin de la suite (inscription publique fermée) — voir e2e/global-setup.ts.
  globalSetup: './e2e/global-setup.ts',
  // La suite complète tourne en parallèle contre un seul serveur : marge par test
  // et par assertion (l'hydratation SSR ralentit sous charge de deux workers).
  timeout: 60_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    baseURL: 'http://127.0.0.1:3000',
    // Navigateur francophone : sans quoi la détection de langue redirige le
    // premier passage à la racine vers /en et casse les sélecteurs français.
    locale: 'fr-CA',
    trace: 'on-first-retry',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
  webServer: {
    command: 'node .output/server/index.mjs',
    url: 'http://127.0.0.1:3000/api/health',
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
    env: {
      NUXT_DATABASE_URL: process.env.NUXT_DATABASE_URL ?? '',
      NUXT_SESSION_PASSWORD:
        process.env.NUXT_SESSION_PASSWORD
        ?? 'e2e-session-password-of-at-least-32-chars!!',
      // Autorise l'import de la page-fixture locale (import.spec) — la garde
      // SSRF reste active partout ailleurs (jamais activé hors tests).
      NUXT_IMPORT_ALLOW_PRIVATE: '1',
    },
  },
})
