import { expect, request, test } from '@playwright/test'
import { BASE_URL } from './env'
import { FRESH_PASSWORD as PASSWORD, registerFreshUser } from './helpers'

// Clés d'API : création depuis les réglages (secret montré une fois), import
// d'une recette par un outil externe (Claude Code) avec la clé, révocation.

// IP distincte par fichier de test (rate limiting).
const IP = '203.0.113.55'
test.use({ extraHTTPHeaders: { 'x-forwarded-for': IP } })

const TITLE = 'One pot haricots crémeux et saucisses'

function recipe() {
  return {
    title: TITLE,
    baseServings: 4,
    prepMinutes: 15,
    cookMinutes: 30,
    sourceUrl: 'https://ici.radio-canada.ca/recettes/9398/one-pot-haricots-cremeux-saucisses',
    ingredients: [
      { quantity: 540, unit: 'ml', name: 'haricots blancs', aisle: 'canned' },
      { quantity: 4, unit: 'unit', name: 'saucisses', aisle: 'butcher' },
    ],
    steps: ['Dorer les saucisses.', 'Ajouter les haricots et mijoter 20 minutes.'],
    tags: ['one pot'],
  }
}

test('clé d\'API : créée dans les réglages, utilisée par un outil externe, puis révoquée', async ({ page }) => {
  await registerFreshUser(page)

  await page.goto('/reglages')
  await page.waitForLoadState('networkidle') // hydratation avant de cliquer
  const panel = page.getByTestId('api-keys')
  await expect(panel.getByTestId('api-keys-empty')).toBeVisible()

  // Création : nom + mot de passe revérifié.
  await panel.getByTestId('api-key-new').click()
  await panel.getByTestId('api-key-name').fill('Claude Code — portable')
  await panel.getByTestId('api-key-password').fill('pas-le-bon-mot-de-passe')
  await panel.getByTestId('api-key-create').click()
  await expect(page.getByRole('alert')).toContainText('Mot de passe')
  await panel.getByTestId('api-key-password').fill(PASSWORD)
  await panel.getByTestId('api-key-create').click()

  // Le secret complet s'affiche une seule fois.
  const tokenText = panel.getByTestId('api-key-token')
  await expect(tokenText).toHaveText(/^hbd_[A-Za-z0-9_-]{43}$/)
  const token = (await tokenText.textContent())!.trim()
  const row = panel.getByTestId('api-key-row')
  await expect(row).toHaveCount(1)
  await expect(row).toContainText('Claude Code — portable')
  await expect(row).toContainText(token.slice(0, 12))
  await expect(row).toContainText('jamais utilisée')

  // L'outil externe enregistre la recette avec la clé (sans cookie).
  const tool = await request.newContext({ baseURL: BASE_URL, extraHTTPHeaders: { 'x-forwarded-for': IP } })
  const created = await tool.post('/api/v1/recipes', {
    data: recipe(),
    headers: { authorization: `Bearer ${token}` },
  })
  expect(created.status(), await created.text()).toBe(201)
  const { id } = await created.json() as { id: string }
  // La clé n'ouvre pas les routes de session.
  const listing = await tool.get('/api/recipes', { headers: { authorization: `Bearer ${token}` } })
  expect(listing.status()).toBe(401)

  // Le secret disparaît une fois confirmé, et ne revient pas au rechargement.
  await panel.getByTestId('api-key-done').click()
  await expect(panel.getByTestId('api-key-reveal')).toHaveCount(0)
  await page.reload()
  await page.waitForLoadState('networkidle')
  await expect(page.getByText(token)).toHaveCount(0)
  await expect(page.getByTestId('api-key-row')).toContainText('dernière utilisation')

  // La recette est bien dans la collection du compte.
  await page.goto(`/recettes/${id}`)
  await expect(page.getByRole('heading', { name: TITLE })).toBeVisible()
  await expect(page.getByText('haricots blancs')).toBeVisible()

  // Révocation : effet immédiat pour l'outil.
  await page.goto('/reglages')
  await page.waitForLoadState('networkidle')
  await page.getByTestId('api-key-revoke').click()
  await page.getByTestId('confirm-dialog-confirm').click()
  await expect(page.getByTestId('settings-notice')).toHaveText('Clé révoquée.')
  await expect(page.getByTestId('api-key-row')).toHaveCount(0)
  const refused = await tool.post('/api/v1/recipes', {
    data: recipe(),
    headers: { authorization: `Bearer ${token}` },
  })
  expect(refused.status()).toBe(401)
  await tool.dispose()
})
