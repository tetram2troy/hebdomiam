import { expect, test, type Page } from '@playwright/test'

// Mode cuisine (wireframe 07) : étapes cochables persistées, quantités à
// l'échelle, « Cuisinée ✓ » qui alimente l'historique.

// IP distincte par fichier de test (rate limiting d'inscription : 5/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.98' } })

async function registerFreshUser(page: Page) {
  const email = `cook-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill('Cuisinière')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mot-de-passe-tres-long-123')
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
}

async function apiRecipe(page: Page, title: string) {
  const status = await page.evaluate(async (payload) => {
    const res = await fetch('/api/recipes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    return res.status
  }, {
    title,
    baseServings: 2, // plan à 4 portions/plat → échelle ×2 bien visible
    ingredients: [{ quantity: 150, unit: 'g', name: `riz pour ${title}`, aisle: 'dry' }],
    steps: [`Préparer ${title}.`, `Cuire ${title}.`],
    tags: [],
  })
  expect(status).toBe(201)
}

test('cuisine : étapes cochées persistées, quantités ×2, Cuisinée ✓', async ({ page }) => {
  await registerFreshUser(page)

  // Sans plan validé → état vide
  await page.goto('/cuisine')
  await expect(page.getByTestId('no-cooking')).toBeVisible()

  // 4 recettes → plan validé (4 plats · 7 j · 1 pers. → 4 portions/plat)
  for (const title of ['Plat A', 'Plat B', 'Plat C', 'Plat D']) {
    await apiRecipe(page, title)
  }
  await page.goto('/planifier')
  // Attendre l'hydratation : un clic trop tôt n'atteint pas le gestionnaire Vue.
  await page.waitForLoadState('networkidle')
  await page.getByTestId('propose').click()
  await page.getByTestId('validate-week').click()
  await expect(page.getByTestId('plan-done')).toBeVisible()

  // Depuis l'accueil, la carte Mode cuisine mène ici
  await page.goto('/')
  await page.waitForLoadState('networkidle')
  await page.getByTestId('cooking-card').click()

  await expect(page.getByTestId('cook-tab')).toHaveCount(4)
  // Échelle : base 2 → cible 4, donc ×2 et 300 g affichés
  await expect(page.getByTestId('scaled-servings')).toHaveText('4 portions')
  await expect(page.getByText('recette de base : 2 → quantités ×2')).toBeVisible()
  const fold = page.locator('.ingfold summary')
  await fold.click()
  await expect(page.getByText('300 g')).toBeVisible()

  // Cocher l'étape 1 — persiste au rechargement (interruptions en cuisine)
  await page.getByTestId('cook-step').first().click()
  await expect(page.getByTestId('cook-step').first()).toHaveAttribute('aria-pressed', 'true')
  await page.reload()
  await expect(page.getByTestId('cook-step').first()).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByTestId('cook-step').nth(1)).toHaveAttribute('aria-pressed', 'false')

  // Cuisinée ✓ : l'onglet passe à ✓ et la sélection avance
  const firstTabText = await page.getByTestId('cook-tab').first().innerText()
  await page.getByTestId('mark-cooked').click()
  await expect(page.getByTestId('cook-tab').filter({ hasText: '✓' })).toHaveCount(1)
  const selectedTab = page.locator('.rtab.on')
  await expect(selectedTab).not.toHaveText(firstTabText)

  // L'accueil reflète la progression
  await page.goto('/')
  await expect(page.getByTestId('cooked-chip')).toHaveText('1 / 4 cuisinées')
})
