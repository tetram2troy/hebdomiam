import { expect, test } from '@playwright/test'
import { FRESH_PASSWORD as PASSWORD, registerFreshUser } from './helpers'

// Réglages du compte (M5) : profil, préférences, changement de mot de passe,
// suppression de compte.

// IP distincte par fichier de test (rate limiting : 5 logins/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.51' } })

test('profil et préférences : nom, unités et rotation persistés', async ({ page }) => {
  await registerFreshUser(page)

  // La nav mène aux réglages
  await page.getByTestId('nav-settings').click()
  await expect(page).toHaveURL(/\/reglages$/)

  // Renommer le compte
  await page.getByRole('button', { name: 'Modifier' }).click()
  await page.getByRole('textbox', { name: 'Nom' }).fill('Nouveau Nom')
  await page.getByTestId('save-name').click()
  await expect(page.getByTestId('profile-name')).toHaveText('Nouveau Nom')

  // Unités impériales + rotation 4 semaines
  await page.getByTestId('units-imperial').click()
  await expect(page.getByTestId('settings-notice')).toBeVisible()
  // Attendre la réponse du PATCH avant de recharger, sinon il part en plein vol.
  const rotationPatched = page.waitForResponse(response =>
    response.url().includes('/api/settings') && response.request().method() === 'PATCH')
  await page.getByRole('button', { name: /Augmenter : Rotation/ }).click()
  await rotationPatched

  // Tout persiste au rechargement, et la nav reflète le nouveau nom (session)
  await page.reload()
  await expect(page.getByTestId('profile-name')).toHaveText('Nouveau Nom')
  await expect(page.getByTestId('nav-settings')).toHaveText('Nouveau Nom')
  await expect(page.getByTestId('units-imperial')).toHaveClass(/on/)
  await expect(page.getByRole('group', { name: /Rotation/ }).locator('.val')).toHaveText('4')

  // Un compte créé par invitation est déjà vérifié (la possession du lien en
  // est la preuve) — le flux « non vérifié » est couvert par courriel.spec.
  await expect(page.getByText('✓ vérifié')).toBeVisible()
  await expect(page.getByTestId('resend-verify')).toHaveCount(0)
})

test('changement de mot de passe : l\'ancien est refusé, le nouveau connecte', async ({ page }) => {
  const email = await registerFreshUser(page)
  const newPassword = 'nouveau-mot-de-passe-solide'

  await page.goto('/reglages')
  await page.waitForLoadState('networkidle') // hydratation avant de cliquer
  await page.getByTestId('change-password').click()
  await page.getByLabel('Mot de passe actuel').fill(PASSWORD)
  await page.getByLabel(/Nouveau mot de passe/).fill(newPassword)
  await page.getByTestId('submit-password').click()
  await expect(page.getByTestId('settings-notice')).toHaveText('Mot de passe changé.')

  await page.getByRole('button', { name: 'Déconnexion' }).first().click()
  await expect(page).toHaveURL(/\/login$/)

  // L'ancien mot de passe ne passe plus
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(PASSWORD)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByRole('alert')).toContainText('invalide')

  // Le nouveau connecte
  await page.getByLabel('Mot de passe').fill(newPassword)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
})

test('suppression de compte : confirmation par mot de passe puis accès révoqué', async ({ page }) => {
  const email = await registerFreshUser(page, 'Éphémère')

  await page.goto('/reglages')
  await page.waitForLoadState('networkidle') // hydratation avant de cliquer
  await page.getByTestId('delete-account').click()
  await page.getByLabel('Mot de passe', { exact: true }).fill(PASSWORD)
  await page.getByTestId('delete-account-confirm').click()
  // Vers la connexion : l'inscription publique est fermée.
  await expect(page).toHaveURL(/\/login$/)

  // Le compte n'existe plus
  await page.goto('/login')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(PASSWORD)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByRole('alert')).toContainText('invalide')
})
