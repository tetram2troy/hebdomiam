import { expect, test } from '@playwright/test'

// Switch de langue FR/EN du header : bascule d'interface pour tout visiteur,
// préférence de compte synchronisée quand on est connecté (langue des courriels).

// IP distincte par fichier de test (rate limiting : 5 inscriptions/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.54' } })

test('visiteur : la bascule EN/FR traduit l\'interface et préfixe l\'URL', async ({ page }) => {
  await page.goto('/login')
  await page.waitForLoadState('networkidle') // hydratation avant de cliquer

  await expect(page.getByTestId('lang-switch')).toHaveText('EN')
  await page.getByTestId('lang-switch').click()

  await expect(page).toHaveURL(/\/en\/login$/)
  await expect(page.getByRole('heading', { name: 'Log in' })).toBeVisible()

  // Retour au français (locale par défaut : URL sans préfixe)
  await expect(page.getByTestId('lang-switch')).toHaveText('FR')
  await page.getByTestId('lang-switch').click()
  await expect(page).toHaveURL(/\/login$/)
  await expect(page.getByRole('heading', { name: 'Connexion' })).toBeVisible()
})

test('connecté : la bascule traduit l\'interface et persiste la préférence du compte', async ({ page }) => {
  const email = `i18n-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill('Bilingue')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mot-de-passe-tres-long-123')
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()

  await page.getByTestId('lang-switch').click()
  await expect(page).toHaveURL(/\/en$/)
  await expect(page.getByTestId('welcome')).toContainText('Hello')

  // La préférence du compte a suivi : les réglages montrent English sélectionné
  await page.goto('/en/reglages')
  await expect(page.getByTestId('lang-en')).toHaveClass(/on/)
})
