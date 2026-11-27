import { expect, test } from '@playwright/test'
import { registerFreshUser } from './helpers'

// Parcours M1 complet : compte frais → collection vide → création d'une recette
// complète → fiche (mise à l'échelle des portions) → recherche/filtres →
// modification → suppression douce.

// IP distincte par fichier de test : le rate limiting applicatif (5/min/IP)
// est un comportement de prod qu'on ne veut pas désactiver.
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.11' } })

test('cycle de vie complet d\'une recette', async ({ page }) => {
  await registerFreshUser(page)

  // Collection vide
  await page.getByRole('link', { name: 'Recettes', exact: true }).click()
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

  // Suppression (modal de confirmation)
  await page.getByTestId('delete-recipe').click()
  await expect(page.getByTestId('confirm-dialog')).toBeVisible()
  await page.getByTestId('confirm-dialog-confirm').click()
  await expect(page.getByTestId('empty-collection')).toBeVisible()
})

test('un utilisateur ne voit pas les recettes d\'un autre (isolation)', async ({ browser }) => {
  // Alice crée une recette
  const alice = await browser.newContext({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.12' } })
  const alicePage = await alice.newPage()
  await registerFreshUser(alicePage)
  await alicePage.goto('/recettes/nouvelle')
  // Attendre l'hydratation : remplir trop tôt laisse v-model réinitialiser les champs.
  await alicePage.waitForLoadState('networkidle')
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

test.describe('mobile (375 px)', () => {
  test.use({ viewport: { width: 375, height: 812 } })

  test('liste et fiche : filtres sur une rangée, feuille « Tous les tags », ingrédients lisibles', async ({ page }) => {
    await registerFreshUser(page)

    // 19 tags sur 2 recettes : enroulés, ils repoussaient la première carte
    // plusieurs écrans plus bas. « végé » est partagé → il arrive en tête.
    const recipes = [
      { title: 'Chili du dimanche', ingredient: 'patates douces moyennes, pelées et coupées en cubes', tags: ['végé', 'mijoté', 'réconfort', 'se congèle', 'hiver', 'haricots', 'tex-mex', 'économique', 'épicé', 'une casserole'] },
      { title: 'Salade de quinoa', ingredient: 'quinoa', tags: ['végé', 'rapide', 'été', 'lunch', 'froid', 'sans cuisson', 'protéines', 'léger', 'citron', 'herbes'] },
    ]
    for (const recipe of recipes) {
      const status = await page.evaluate(async (payload) => {
        const res = await fetch('/api/recipes', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        })
        return res.status
      }, {
        title: recipe.title,
        tags: recipe.tags,
        baseServings: 4,
        ingredients: [{ quantity: 2, unit: 'unit', name: recipe.ingredient, aisle: 'produce' }],
        steps: ['Tout mélanger.'],
      })
      expect(status).toBe(201)
    }

    await page.goto('/recettes')
    await page.waitForLoadState('networkidle') // hydratation avant de cliquer
    await expect(page.getByTestId('recipe-card')).toHaveCount(2)

    // Une seule rangée, et la première recette est visible sans défiler.
    const filters = page.getByRole('group', { name: 'Filtres' })
    expect((await filters.boundingBox())!.height).toBeLessThanOrEqual(64)
    expect((await page.getByTestId('recipe-card').first().boundingBox())!.y).toBeLessThan(420)
    // La rangée défile dans son cadre : la page, elle, ne déborde pas.
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)
    expect(overflow).toBeLessThanOrEqual(0)

    // Survol (qui reste « collé » après un toucher sur mobile) : la chip garde
    // un fond quasi transparent — `button:hover` la peignait en vert primaire
    // opaque, texte sombre dessus.
    const firstChip = filters.getByRole('button', { name: 'végé' })
    await firstChip.hover()
    await expect.poll(async () => {
      const background = await firstChip.evaluate(el => getComputedStyle(el).backgroundColor)
      const alpha = /[/,]\s*([\d.]+)\)$/.exec(background)?.[1]
      return background.startsWith('rgb(') ? 1 : Number(alpha ?? 1)
    }).toBeLessThan(0.2)

    // « Tous · 19 » ouvre la feuille : tous les tags, du plus utilisé au moins utilisé.
    const allTags = page.getByTestId('all-tags')
    await expect(allTags).toHaveText('Tous · 19')
    await allTags.click()
    const sheet = page.getByTestId('tag-sheet')
    await expect(sheet).toBeVisible()
    const options = sheet.getByTestId('tag-sheet-option')
    await expect(options).toHaveCount(19)
    await expect(options.first()).toHaveText('végé · 2')

    // Recherche sans accent → le tag accentué. La feuille garde sa hauteur :
    // tassée en bas d'écran, elle passerait sous le clavier du téléphone.
    const sheetBefore = (await sheet.boundingBox())!
    await sheet.getByTestId('tag-sheet-search').fill('epice')
    await expect(options).toHaveCount(1)
    expect((await sheet.boundingBox())!.height).toBe(sheetBefore.height)
    expect((await options.first().boundingBox())!.y).toBeLessThan(400)
    // Le choisir ferme la feuille.
    await options.first().click()
    await expect(sheet).toBeHidden()

    // Le filtre s'applique et le tag actif est ramené en tête de rangée, à l'écran.
    await expect(page.getByTestId('recipe-card')).toHaveCount(1)
    const active = filters.getByRole('button', { name: 'épicé', pressed: true })
    await expect(active).toBeInViewport({ ratio: 1 })

    // « Tout effacer » reste à portée, juste après le tag actif.
    const clear = filters.getByRole('button', { name: 'Tout effacer' })
    await expect(clear).toBeInViewport({ ratio: 1 })

    // L'ordre est celui du DOM, pas un réordonnancement CSS : la tabulation et
    // les lecteurs d'écran suivent ce qu'on voit (« Tous », tag actif, effacer).
    const rowButtons = filters.getByRole('button')
    await expect(rowButtons.nth(0)).toHaveText('Tous · 19')
    await expect(rowButtons.nth(1)).toHaveText('épicé ✓')
    await expect(rowButtons.nth(2)).toHaveText('Tout effacer')

    // Au clavier : désactiver le tag le renvoie à sa place — le focus ne se
    // perd pas, il revient au premier bouton de la rangée.
    await active.focus()
    await page.keyboard.press('Enter')
    await expect(page.getByTestId('recipe-card')).toHaveCount(2)
    await expect(rowButtons.nth(0)).toBeFocused()

    // Fiche recette en une colonne : le rayon passe sous le nom de l'ingrédient
    // (à côté, il le repliait sur cinq ou six lignes), et le lien de retour
    // est une vraie cible tactile.
    await page.getByTestId('recipe-card').filter({ hasText: 'Chili du dimanche' }).click()
    await expect(page.getByTestId('recipe-title')).toHaveText('Chili du dimanche')
    const row = page.locator('.ingredients li').first()
    const name = (await row.locator('.name').boundingBox())!
    const aisle = (await row.locator('.aisle').boundingBox())!
    expect(name.width).toBeGreaterThanOrEqual(180)
    expect(aisle.y).toBeGreaterThanOrEqual(name.y + name.height - 1)
    const back = (await page.getByRole('link', { name: '← Mes recettes' }).boundingBox())!
    expect(back.height).toBeGreaterThanOrEqual(48)
  })
})

test('desktop (1280 px) : filtres sur une rangée, « Tous les tags » en dialogue centré', async ({ page }) => {
  await registerFreshUser(page)

  // 30 tags sur 2 recettes : enroulés, ils occupaient trois rangées au-dessus
  // de la grille. « végé » est partagé → il arrive en tête.
  const recipes = [
    { title: 'Chili du dimanche', tags: ['végé', 'mijoté', 'réconfort', 'se congèle', 'hiver', 'haricots', 'tex-mex', 'économique', 'épicé', 'une casserole', 'mijoteuse', 'boîte à lunch', 'légumineuses', 'sans gluten', 'restes', 'festif'] },
    { title: 'Salade de quinoa', tags: ['végé', 'rapide', 'été', 'lunch', 'froid', 'sans cuisson', 'protéines', 'léger', 'citron', 'herbes', 'salade', 'pique-nique', 'accompagnement', 'croquant', 'vinaigrette'] },
  ]
  for (const recipe of recipes) {
    const status = await page.evaluate(async (payload) => {
      const res = await fetch('/api/recipes', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      })
      return res.status
    }, {
      ...recipe,
      baseServings: 4,
      ingredients: [{ quantity: 1, unit: 'unit', name: `base ${recipe.title}`, aisle: 'other' }],
      steps: ['Tout mélanger.'],
    })
    expect(status).toBe(201)
  }

  await page.goto('/recettes')
  await page.waitForLoadState('networkidle') // hydratation avant de cliquer
  await expect(page.getByTestId('recipe-card')).toHaveCount(2)

  // Une seule rangée, sans défilement horizontal de la page.
  const filters = page.getByRole('group', { name: 'Filtres' })
  expect((await filters.boundingBox())!.height).toBeLessThanOrEqual(64)
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)
  expect(overflow).toBeLessThanOrEqual(0)

  // « Tous · 30 » ouvre un dialogue centré, pas une feuille collée en bas.
  await expect(page.getByTestId('all-tags')).toHaveText('Tous · 30')
  await page.getByTestId('all-tags').click()
  const sheet = page.getByTestId('tag-sheet')
  await expect(sheet).toBeVisible()
  const box = (await sheet.boundingBox())!
  expect(box.width).toBeLessThanOrEqual(560)
  expect(Math.abs((box.x + box.width / 2) - 640)).toBeLessThanOrEqual(10) // marge : barre de défilement
  expect(box.y).toBeGreaterThan(40)
  expect(box.y + box.height).toBeLessThan(720 - 40)

  // Un tag de fin de liste, hors de la rangée visible : choisi dans le
  // dialogue, il filtre et remonte en tête de rangée.
  await sheet.getByTestId('tag-sheet-search').fill('vinai')
  await sheet.getByTestId('tag-sheet-option').click()
  await expect(sheet).toBeHidden()
  await expect(page.getByTestId('recipe-card')).toHaveCount(1)
  await expect(filters.getByRole('button', { name: 'vinaigrette', pressed: true })).toBeInViewport({ ratio: 1 })

  // Échap ferme sans rien changer.
  await page.getByTestId('all-tags').click()
  await expect(sheet).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(sheet).toBeHidden()
  await expect(page.getByTestId('recipe-card')).toHaveCount(1)
})
