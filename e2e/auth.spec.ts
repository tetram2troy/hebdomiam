import { expect, test } from '@playwright/test'
import { FRESH_PASSWORD, registerFreshUser } from './helpers'

// Parcours critique M0, révisé pour le multi-utilisateur géré : l'inscription
// publique est fermée (les comptes naissent par invitation admin) ; le cycle
// connexion/déconnexion reste le contrat de base.

// IP distincte par fichier de test (rate limiting applicatif : 5/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.31' } })

test('l\'inscription publique est fermée : page et API la refusent', async ({ page }) => {
  await page.goto('/register')
  await page.waitForLoadState('networkidle')

  // La page remplace le formulaire par l'explication et un lien de connexion
  await expect(page.getByTestId('register-closed')).toBeVisible()
  await expect(page.getByLabel('Courriel')).toHaveCount(0)

  // La défense réelle est côté serveur : le POST direct est refusé
  const response = await page.request.post('/api/auth/register', {
    data: { name: 'Intruse', email: 'intruse@example.com', password: 'mot-de-passe-tres-long-123' },
  })
  expect(response.status()).toBe(403)
})

test('compte invité : déconnexion puis reconnexion', async ({ page }) => {
  const email = await registerFreshUser(page, 'Testeur M0')
  await expect(page.getByTestId('welcome')).toContainText('Testeur M0')

  await page.getByRole('button', { name: 'Déconnexion' }).click()
  await expect(page).toHaveURL(/\/login$/)

  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(FRESH_PASSWORD)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByTestId('welcome')).toContainText('Testeur M0')
})

test('refuse un mauvais mot de passe', async ({ page }) => {
  const email = await registerFreshUser(page, 'Prudente M0')
  await page.getByRole('button', { name: 'Déconnexion' }).click()
  await expect(page).toHaveURL(/\/login$/)

  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mauvais-mot-de-passe-999')
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByRole('alert')).toContainText('invalide')
  await expect(page).toHaveURL(/\/login$/)
})
