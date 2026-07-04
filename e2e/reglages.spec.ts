import { expect, test, type Page } from '@playwright/test'

// Réglages du compte (M5) : profil, préférences, changement de mot de passe,
// suppression de compte.

// IP distincte par fichier de test (rate limiting : 5 inscriptions et 5 logins/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.51' } })

const PASSWORD = 'mot-de-passe-tres-long-123'

async function registerFreshUser(page: Page, name = 'Régleuse') {
  const email = `reglages-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill(name)
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(PASSWORD)
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
  return email
}

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

  // Le courriel est marqué non vérifié tant que le lien n'est pas cliqué
  await expect(page.getByText('non vérifié')).toBeVisible()
  await expect(page.getByTestId('resend-verify')).toBeVisible()
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
  await expect(page).toHaveURL(/\/register$/)

  // Le compte n'existe plus
  await page.goto('/login')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(PASSWORD)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByRole('alert')).toContainText('invalide')
})
