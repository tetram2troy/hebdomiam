import { expect, test, type Page } from '@playwright/test'

// M2 — le rituel du vendredi : paramètres → suggestions (✓/↻/remplacer) →
// validation → accueil (semaine planifiée) → « Cuisinée ✓ » → rotation.

// IP distincte par fichier de test (rate limiting d'inscription : 5/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.41' } })

async function registerFreshUser(page: Page) {
  const email = `m2-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  // Attendre l'hydratation : un clic trop tôt déclenche la soumission native du
  // formulaire SSR (rechargement à vide) au lieu du gestionnaire Vue.
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill('Planificatrice')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mot-de-passe-tres-long-123')
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
}

/** Seed rapide par l'API, depuis la page (cookies de session natifs) —
    la création par l'UI est couverte par recettes.spec. */
async function apiRecipe(page: Page, title: string, tags: string[] = []) {
  const status = await page.evaluate(async (payload) => {
    const res = await fetch('/api/recipes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    return res.status
  }, {
    title,
    baseServings: 4,
    prepMinutes: 10,
    cookMinutes: 20,
    ingredients: [{ quantity: 1, unit: 'unit', name: `ingrédient ${title}`, aisle: 'other' }],
    steps: [`Préparer ${title}.`],
    tags,
  })
  expect(status).toBe(201)
}

test('rituel du vendredi complet : paramètres → suggestions → validation → cuisinée', async ({ page }) => {
  await registerFreshUser(page)

  // Accueil, état A : rien de prévu
  await expect(page.getByTestId('no-plan')).toBeVisible()

  // 6 recettes (4 suggérées + 2 de réserve pour ↻ et le remplacement)
  const titles = ['Chili sans viande', 'Soupe orge', 'Poulet au beurre', 'Tofu Tao', 'Dahl corail', 'Lasagne courge']
  for (const [i, title] of titles.entries()) {
    await apiRecipe(page, title, [`tag-${i}`])
  }

  // Étape 1 — paramètres par défaut (4 plats · 1 pers. · 7 jours → 14 portions)
  await page.getByTestId('nav-plan').click()
  await expect(page.getByTestId('plan-params')).toBeVisible()
  await expect(page.getByText('≈ 14 portions à cuisiner')).toBeVisible()
  await page.getByTestId('propose').click()

  // Étape 2 — 4 suggestions
  await expect(page.getByTestId('plan-suggestions')).toBeVisible()
  const cards = page.getByTestId('suggestion-card')
  await expect(cards).toHaveCount(4)
  await expect(page.getByTestId('accepted-count')).toHaveText('0 / 4 validées')

  // ✓ Valider la première — conformité design : contour primaire sur la carte validée
  await page.getByTestId('accept').first().click()
  await expect(page.getByTestId('accepted-count')).toHaveText('1 / 4 validées')
  const acceptedCard = cards.filter({ hasText: 'Annuler la validation' }).first()
  const borderColor = await acceptedCard.evaluate(el => getComputedStyle(el).borderColor)
  expect(borderColor).toBe('rgb(30, 122, 61)')

  // ↻ Échanger une suggestion — le titre change pour une recette hors plan
  const titlesBefore = await cards.locator('.title').allInnerTexts()
  await page.getByTestId('swap').first().click()
  await expect(async () => {
    const titlesAfter = await cards.locator('.title').allInnerTexts()
    expect(titlesAfter).toHaveLength(4)
    expect(titlesAfter).not.toEqual(titlesBefore)
  }).toPass()

  // Remplacer par une recette précise (recherche)
  const titlesNow = await cards.locator('.title').allInnerTexts()
  const spare = titles.find(t => !titlesNow.includes(t))!
  await page.getByTestId('open-replace').first().click()
  await expect(page.getByTestId('replace-panel')).toBeVisible()
  await page.getByLabel('Chercher dans mes recettes').fill(spare)
  await expect(page.getByTestId('choose-option')).toHaveCount(1) // recherche débouncée appliquée
  await page.getByTestId('choose-option').click()
  await expect(page.getByTestId('replace-panel')).toBeHidden()
  await expect(page.getByTestId('accepted-count')).toHaveText('2 / 4 validées')
  await expect(cards.locator('.title').filter({ hasText: spare })).toHaveCount(1)

  // Mode « Me suggérer autre chose » : la feuille propose la rotation
  await page.getByTestId('open-replace').first().click()
  await page.getByTestId('mode-suggest').click()
  await expect(page.getByText('Suggestions selon ta rotation')).toBeVisible()
  await page.getByRole('button', { name: '✕ Fermer' }).click()

  // Étape 3 — valider la semaine (les suggestions restantes sont acceptées)
  await page.getByTestId('validate-week').click()
  await expect(page.getByTestId('plan-done')).toBeVisible()
  await page.getByTestId('back-home').click()

  // Accueil, état B : semaine planifiée
  await expect(page.getByTestId('week-plan')).toBeVisible()
  await expect(page.getByTestId('cooked-chip')).toHaveText('0 / 4 cuisinées')
  await expect(page.getByTestId('plan-card')).toHaveCount(4)
  await expect(page.getByText('À cuisiner').first()).toBeVisible()

  // Cuisinée ✓ — alimente l'historique
  await page.getByTestId('mark-cooked').first().click()
  await expect(page.getByTestId('cooked-chip')).toHaveText('1 / 4 cuisinées')
  await expect(page.getByText('✓ Cuisinée').first()).toBeVisible()
})

test('garde-fous et rotation : collection trop petite, puis exclusion des cuisinées', async ({ page }) => {
  await registerFreshUser(page)

  // Collection trop petite → proposition bloquée avec explication
  await apiRecipe(page, 'Seule recette A')
  await apiRecipe(page, 'Seule recette B')
  await page.goto('/planifier')
  await expect(page.getByTestId('not-enough')).toBeVisible()
  await expect(page.getByTestId('propose')).toBeDisabled()

  // On complète à 5 recettes → 4 plats proposés
  await apiRecipe(page, 'Recette C')
  await apiRecipe(page, 'Recette D')
  await apiRecipe(page, 'Jamais cuisinée E')
  await page.reload()
  // Attendre l'hydratation : un clic trop tôt n'atteint pas le gestionnaire Vue.
  await page.waitForLoadState('networkidle')
  await page.getByTestId('propose').click()
  await expect(page.getByTestId('suggestion-card')).toHaveCount(4)

  // Valider et tout cuisiner
  await page.getByTestId('validate-week').click()
  await page.getByTestId('back-home').click()
  for (let i = 0; i < 4; i++) {
    await page.getByTestId('mark-cooked').first().click()
    await expect(page.getByTestId('cooked-chip')).toHaveText(`${i + 1} / 4 cuisinées`)
  }
  await expect(page.getByTestId('all-cooked')).toBeVisible()

  // Replanifier : la recette jamais cuisinée doit être priorisée par la rotation
  const cookedTitles = await page.getByTestId('plan-card').locator('.title').allInnerTexts()
  const neverCooked = ['Seule recette A', 'Seule recette B', 'Recette C', 'Recette D', 'Jamais cuisinée E']
    .find(t => !cookedTitles.includes(t))!

  await page.getByTestId('replan').click()
  await page.getByTestId('confirm-dialog-confirm').click()
  await expect(page.getByTestId('plan-params')).toBeVisible()
  await page.getByTestId('propose').click()
  await expect(page.getByTestId('suggestion-card')).toHaveCount(4)
  await expect(page.getByTestId('suggestion-card').locator('.title').filter({ hasText: neverCooked }))
    .toHaveCount(1)
})
