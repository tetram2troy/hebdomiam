import { expect, test, type Page } from '@playwright/test'

// Parcours M1 complet : inscription → collection vide → création d'une recette
// complète → fiche (mise à l'échelle des portions) → recherche/filtres →
// modification → suppression douce.

// IP distincte par fichier de test : le rate limiting d'inscription (5/min/IP)
// est un comportement de prod qu'on ne veut pas désactiver.
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.11' } })

async function registerFreshUser(page: Page) {
  const email = `m1-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  await page.getByLabel('Nom').fill('Testeuse M1')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mot-de-passe-tres-long-123')
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toContainText('Testeuse M1')
}

test('cycle de vie complet d\'une recette', async ({ page }) => {
  await registerFreshUser(page)

  // Collection vide
  await page.getByRole('link', { name: 'Recettes' }).click()
  await expect(page.getByTestId('empty-collection')).toBeVisible()

  // Création
  await page.getByTestId('new-recipe').click()
  await expect(page.getByRole('heading', { name: 'Nouvelle recette' })).toBeVisible()

  await page.getByLabel('Titre *').fill('Chili de lentilles à la mijoteuse')
  await page.getByLabel('Description').fill('Un classique réconfortant.')
  await page.getByLabel('Portions de base *').fill('4')
  await page.getByLabel('Préparation (min)').fill('15')
  await page.getByLabel('Cuisson (min)').fill('30')

  // Ingrédient 1 : 500 g de lentilles, produits secs
  const row1 = page.locator('.ingredient-row').first()
  await row1.getByLabel('Quantité').fill('500')
  await row1.getByLabel('Unité').selectOption('g')
  await row1.getByLabel('Nom').fill('lentilles vertes')
  await row1.getByLabel('Rayon').selectOption('dry')

  // Ingrédient 2 : 2 oignons (unité « unité »), fruits et légumes
  await page.getByRole('button', { name: 'Ajouter un ingrédient' }).click()
  const row2 = page.locator('.ingredient-row').nth(1)
  await row2.getByLabel('Quantité').fill('2')
  await row2.getByLabel('Unité').selectOption('unit')
  await row2.getByLabel('Nom').fill('oignons jaunes')
  await row2.getByLabel('Rayon').selectOption('produce')

  // Étapes
  await page.getByLabel('Étape 1', { exact: true }).fill('Hacher les oignons et les faire revenir.')
  await page.getByRole('button', { name: 'Ajouter une étape' }).click()
  await page.getByLabel('Étape 2', { exact: true }).fill('Tout verser dans la mijoteuse et cuire 3 h.')

  // Tags (Entrée pour ajouter)
  const tagInput = page.getByLabel('Tags')
  await tagInput.fill('végé')
  await tagInput.press('Enter')
  await tagInput.fill('mijoteuse')
  await tagInput.press('Enter')
  await expect(page.getByRole('button', { name: 'Retirer le tag végé' })).toBeVisible()

  await page.getByLabel('Se congèle / se conserve bien').check()
  await page.getByTestId('recipe-submit').click()

  // Fiche
  await expect(page.getByTestId('recipe-title')).toHaveText('Chili de lentilles à la mijoteuse')
  await expect(page.getByText('lentilles vertes')).toBeVisible()
  await expect(page.getByText('❄ se congèle')).toBeVisible()
  await expect(page.getByText('Total 45 min')).toBeVisible()

  // Mise à l'échelle : 4 → 5 portions, 500 g → 625 g, 2 oignons → 2,5
  const ingredients = page.locator('.ingredients')
  await expect(ingredients).toContainText('500 g')
  await page.getByRole('button', { name: 'Augmenter : Portions' }).click()
  await expect(ingredients).toContainText('625 g')
  await expect(ingredients).toContainText('2,5')

  // Liste : carte, recherche et filtre par tag
  await page.getByRole('link', { name: '← Mes recettes' }).click()
  await expect(page.getByTestId('recipe-card')).toHaveCount(1)

  const search = page.getByLabel('Chercher une recette')
  await search.fill('bourguignon')
  await expect(page.getByTestId('no-results')).toBeVisible()
  await page.getByTestId('no-results').getByRole('button', { name: 'Tout effacer' }).click()

  await search.fill('lentilles') // trouve par ingrédient
  await expect(page.getByTestId('recipe-card')).toHaveCount(1)
  await search.clear()

  await page.getByRole('button', { name: 'végé' }).click()
  await expect(page.getByTestId('recipe-card')).toHaveCount(1)

  // Modification
  await page.getByTestId('recipe-card').click()
  await page.getByTestId('edit-recipe').click()
  await expect(page.getByRole('heading', { name: 'Modifier la recette' })).toBeVisible()
  const title = page.getByLabel('Titre *')
  await expect(title).toHaveValue('Chili de lentilles à la mijoteuse')
  await title.fill('Chili revisité')
  await page.getByTestId('recipe-submit').click()
  await expect(page.getByTestId('recipe-title')).toHaveText('Chili revisité')

  // Suppression (avec confirmation native)
  page.on('dialog', dialog => dialog.accept())
  await page.getByTestId('delete-recipe').click()
  await expect(page.getByTestId('empty-collection')).toBeVisible()
})

test('un utilisateur ne voit pas les recettes d\'un autre (isolation)', async ({ browser }) => {
  // Alice crée une recette
  const alice = await browser.newContext({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.12' } })
  const alicePage = await alice.newPage()
  await registerFreshUser(alicePage)
  await alicePage.goto('/recettes/nouvelle')
  await alicePage.getByLabel('Titre *').fill('Secret d\'Alice')
  await alicePage.getByLabel('Portions de base *').fill('2')
  const row = alicePage.locator('.ingredient-row').first()
  await row.getByLabel('Quantité').fill('1')
  await row.getByLabel('Nom').fill('ingrédient secret')
  await alicePage.getByLabel('Étape 1', { exact: true }).fill('Étape secrète.')
  await alicePage.getByTestId('recipe-submit').click()
  await expect(alicePage.getByTestId('recipe-title')).toBeVisible()
  const recipeUrl = alicePage.url()
  await alice.close()

  // Bob, connecté, ne voit rien : liste vide et accès direct refusé
  const bob = await browser.newContext({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.13' } })
  const bobPage = await bob.newPage()
  await registerFreshUser(bobPage)
  await bobPage.goto('/recettes')
  await expect(bobPage.getByTestId('empty-collection')).toBeVisible()
  await bobPage.goto(recipeUrl)
  await expect(bobPage.getByText('Introuvable', { exact: false })).toBeVisible()
  await bob.close()
})
