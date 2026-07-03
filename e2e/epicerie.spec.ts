import { expect, test, type Page } from '@playwright/test'

// M3 — liste d'épicerie : génération à la validation, agrégation à l'écran,
// cochage mode magasin (cochés vers le bas), article libre, correction de rayon.

// IP distincte par fichier de test (rate limiting d'inscription : 5/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.71' } })

async function registerFreshUser(page: Page) {
  const email = `m3-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill('Épicière')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mot-de-passe-tres-long-123')
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
}

async function apiRecipe(page: Page, title: string, ingredients: Array<{ name: string, quantity: number, unit: string | null, aisle: string }>) {
  const status = await page.evaluate(async (payload) => {
    const res = await fetch('/api/recipes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    return res.status
  }, { title, baseServings: 4, ingredients, steps: [`Préparer ${title}.`], tags: [] })
  expect(status).toBe(201)
}

test('du plan validé au panier plein : agrégation, cochage, article libre, rayon', async ({ page }) => {
  await registerFreshUser(page)

  // Pas de liste tant que rien n'est validé
  await page.goto('/epicerie')
  await expect(page.getByTestId('no-list')).toBeVisible()

  // 4 recettes (portions de base 4 = cible du plan → échelle ×1, chiffres nets) :
  // riz fusionne (200+150), beurre fusionne et passe en kg (800+700), tomates
  // reste en 2 lignes (unités incompatibles).
  await apiRecipe(page, 'Poulet au riz', [
    { name: 'riz basmati', quantity: 200, unit: 'g', aisle: 'dry' },
  ])
  await apiRecipe(page, 'Tofu au riz', [
    { name: 'riz basmati', quantity: 150, unit: 'g', aisle: 'dry' },
  ])
  await apiRecipe(page, 'Tomates au beurre', [
    { name: 'tomates', quantity: 2, unit: 'unit', aisle: 'produce' },
    { name: 'beurre', quantity: 800, unit: 'g', aisle: 'dairy' },
  ])
  await apiRecipe(page, 'Sauce tomate riche', [
    { name: 'tomates', quantity: 400, unit: 'g', aisle: 'produce' },
    { name: 'beurre', quantity: 700, unit: 'g', aisle: 'dairy' },
  ])

  // Planifier et valider la semaine → la liste naît avec la validation
  await page.goto('/planifier')
  await page.getByTestId('propose').click()
  await expect(page.getByTestId('suggestion-card')).toHaveCount(4)
  await page.getByTestId('validate-week').click()
  await page.getByTestId('see-grocery').click()

  // Agrégation à l'écran
  await expect(page.getByTestId('progress')).toHaveText('0 / 4 dans le panier')
  const rizRow = page.getByTestId('grocery-item').filter({ hasText: 'riz basmati' })
  await expect(rizRow).toContainText('350 g')
  await expect(page.getByTestId('grocery-item').filter({ hasText: 'beurre' })).toContainText('1,5 kg')
  await expect(page.getByTestId('grocery-item').filter({ hasText: 'tomates' })).toHaveCount(2)

  // Rayons dans l'ordre du magasin, en-têtes présents
  await expect(page.getByTestId('aisle-produce')).toBeVisible()
  await expect(page.getByTestId('aisle-dairy')).toBeVisible()
  await expect(page.getByTestId('aisle-dry')).toBeVisible()

  // Cible tactile mode magasin : rangée ≥ 56 px
  const rowBox = await page.getByTestId('grocery-item').first().boundingBox()
  expect(rowBox!.height).toBeGreaterThanOrEqual(56)

  // Cocher le riz → il descend « Dans le panier », la progression suit
  await page.getByRole('button', { name: 'Cocher riz basmati' }).click()
  await expect(page.getByTestId('progress')).toHaveText('1 / 4 dans le panier')
  await expect(page.getByTestId('cart-item')).toHaveCount(1)
  await expect(page.getByTestId('cart-item')).toContainText('riz basmati')
  await expect(page.getByTestId('grocery-item').filter({ hasText: 'riz basmati' })).toHaveCount(0)

  // Article libre → rayon « Autre », badge « libre »
  await page.getByLabel('Ajouter un article').fill('Essuie-tout')
  await page.getByTestId('add-item').click()
  await expect(page.getByTestId('progress')).toHaveText('1 / 5 dans le panier')
  const manualRow = page.getByTestId('grocery-item').filter({ hasText: 'Essuie-tout' })
  await expect(manualRow).toContainText('libre')
  await expect(page.getByTestId('aisle-other')).toBeVisible()

  // Correction de rayon via la feuille ✎ → l'article déménage
  await page.getByRole('button', { name: 'Modifier Essuie-tout' }).click()
  await expect(page.getByTestId('edit-sheet')).toBeVisible()
  await page.getByTestId('aisle-select').selectOption('household')
  await expect(page.getByTestId('aisle-household')).toBeVisible()
  await expect(page.getByTestId('aisle-other')).toBeHidden()

  // Provenance : la feuille du beurre montre l'agrégation
  await page.getByRole('button', { name: '✕ Fermer' }).click()
  await page.getByRole('button', { name: 'Modifier beurre' }).click()
  await expect(page.getByTestId('edit-sheet')).toContainText('Agrégé de :')
  await expect(page.getByTestId('edit-sheet')).toContainText('Tomates au beurre (800 g)')

  // Retirer l'article libre
  await page.getByRole('button', { name: '✕ Fermer' }).click()
  await page.getByRole('button', { name: 'Modifier Essuie-tout' }).click()
  await page.getByTestId('remove-item').click()
  await expect(page.getByTestId('progress')).toHaveText('1 / 4 dans le panier')

  // Décocher depuis le panier → l'article remonte
  await page.getByRole('button', { name: 'Décocher riz basmati' }).click()
  await expect(page.getByTestId('progress')).toHaveText('0 / 4 dans le panier')
  await expect(page.getByTestId('cart-item')).toHaveCount(0)

  // La carte de l'accueil reflète le restant
  await page.goto('/')
  await expect(page.getByTestId('grocery-card')).toContainText('4 articles restants')
})
