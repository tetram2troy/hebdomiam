import { defineConfig, devices } from '@playwright/test'
import { BASE_URL } from './e2e/env'

export default defineConfig({
  testDir: './e2e',
  // Démarre le serveur testé (API Rust + front généré) puis sème le compte
  // admin de la suite (inscription publique fermée) — voir e2e/global-setup.ts.
  globalSetup: './e2e/global-setup.ts',
  // La suite complète tourne contre un seul serveur : marge par test et par assertion.
  timeout: 60_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    baseURL: BASE_URL,
    // Navigateur francophone : sans quoi la détection de langue redirige le
    // premier passage à la racine vers /en et casse les sélecteurs français.
    locale: 'fr-CA',
    trace: 'on-first-retry',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
})
