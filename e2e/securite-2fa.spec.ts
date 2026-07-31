import { expect, test, type Page } from '@playwright/test'
import { generateSync } from 'otplib'
import { FRESH_PASSWORD as PASSWORD, registerFreshUser } from './helpers'

// 2FA TOTP (M5) : activation depuis les réglages, connexion en deux étapes,
// code de secours à usage unique, désactivation par mot de passe.

// IP distincte par fichier de test (rate limiting login et 2FA : 5/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.52' } })

async function login(page: Page, email: string) {
  await page.goto('/login')
  await page.waitForLoadState('networkidle') // hydratation avant de remplir
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(PASSWORD)
  await page.getByRole('button', { name: 'Se connecter' }).click()
}

test('2FA : activation, connexion avec code, code de secours puis désactivation', async ({ page }) => {
  test.setTimeout(90_000) // parcours long : 4 connexions successives

  const email = await registerFreshUser(page, 'Prudente')

  // ── Activation depuis les réglages ────────────────────────────────────────
  await page.goto('/reglages')
  await page.waitForLoadState('networkidle') // hydratation avant de cliquer
  await page.getByTestId('totp-enable').click()
  await expect(page.getByTestId('totp-setup')).toBeVisible()
  await expect(page.locator('.qr svg')).toBeVisible() // QR à scanner

  // Le secret affiché permet la saisie manuelle — on s'en sert comme l'app d'authentification
  const secretText = await page.getByTestId('totp-secret').innerText()
  const secret = /([A-Z2-7]{16,})/.exec(secretText)?.[1]
  expect(secret, 'secret base32 affiché').toBeTruthy()

  await page.getByLabel('Code', { exact: true }).fill(generateSync({ secret: secret! }))
  await page.getByTestId('totp-confirm').click()

  // Code de secours montré une seule fois
  const backupText = await page.getByTestId('totp-backup').innerText()
  const backupCode = /([0-9a-f]{12})/.exec(backupText)?.[1]
  expect(backupCode, 'code de secours affiché').toBeTruthy()

  // ── Connexion en deux étapes ──────────────────────────────────────────────
  await page.getByRole('button', { name: 'Déconnexion' }).first().click()
  await login(page, email)

  // Mot de passe seul insuffisant : l'étape code s'affiche, pas de session
  await expect(page.getByTestId('totp-login-code')).toBeVisible()

  // Mauvais code refusé
  await page.getByTestId('totp-login-code').fill('000000')
  await page.getByTestId('totp-login-submit').click()
  await expect(page.getByRole('alert')).toContainText('Code invalide')

  // Bon code → connecté
  await page.getByTestId('totp-login-code').fill(generateSync({ secret: secret! }))
  await page.getByTestId('totp-login-submit').click()
  await expect(page.getByTestId('welcome')).toBeVisible()

  // ── Code de secours : entre une fois, puis est consommé ──────────────────
  await page.getByRole('button', { name: 'Déconnexion' }).first().click()
  await login(page, email)
  await page.getByTestId('totp-login-code').fill(backupCode!)
  await page.getByTestId('totp-login-submit').click()
  await expect(page.getByTestId('welcome')).toBeVisible()

  // ── Désactivation protégée par mot de passe ───────────────────────────────
  await page.goto('/reglages')
  await page.waitForLoadState('networkidle') // hydratation avant de cliquer
  await page.getByTestId('totp-disable').click()
  await page.getByLabel('Mot de passe', { exact: true }).fill(PASSWORD)
  await page.getByTestId('totp-disable-confirm').click()
  await expect(page.getByTestId('totp-enable')).toBeVisible() // état « désactivée »

  // La connexion redevient directe (plus d'étape code)
  await page.getByRole('button', { name: 'Déconnexion' }).first().click()
  await login(page, email)
  await expect(page.getByTestId('welcome')).toBeVisible()
})
