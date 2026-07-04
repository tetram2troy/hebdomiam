import { expect, test, type Page } from '@playwright/test'

// M4 — import par URL : pipeline complet (fetch → schema.org → brouillon)
// contre une page-fixture servie par l'app (NUXT_IMPORT_ALLOW_PRIVATE=1 en test).

// IP distincte par fichier de test (rate limiting d'inscription : 5/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.95' } })

async function registerFreshUser(page: Page) {
  const email = `m4-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill('Importatrice')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mot-de-passe-tres-long-123')
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
}

test('import par URL : extraction → brouillon à relire → enregistrement', async ({ page, baseURL }) => {
  await registerFreshUser(page)

  // Entrée par la liste des recettes
  await page.goto('/recettes')
  await page.getByTestId('import-recipe').click()
  await expect(page.getByRole('heading', { name: 'Importer une recette' })).toBeVisible()

  // Extraction de la fixture via le pipeline complet
  await page.getByTestId('import-url').fill(`${baseURL}/test-fixtures/recette-fixture.html`)
  await page.getByTestId('extract-url').click()

  // Brouillon : bandeau mandarine + avertissements + formulaire pré-rempli
  await expect(page.getByTestId('draft-banner')).toBeVisible()
  await expect(page.getByTestId('draft-banner')).toContainText('Brouillon extrait')
  await expect(page.getByTestId('draft-banner')).toContainText('recette-fixture.html')
  // « Farine grillée au goût » n'a pas de quantité → avertissement
  await expect(page.getByTestId('draft-warnings')).toContainText('Farine grillée au goût')

  await expect(page.getByLabel('Titre *')).toHaveValue('Ragoût de boulettes de grand-maman')
  await expect(page.getByLabel('Portions de base *')).toHaveValue('6')
  await expect(page.getByLabel('Préparation (min)')).toHaveValue('25')
  await expect(page.getByLabel('Cuisson (min)')).toHaveValue('60')

  // La photo extraite est visible dans le brouillon et modifiable
  await expect(page.getByTestId('image-url')).toHaveValue('https://exemple.test/photos/ragout-de-boulettes.jpg')

  // Les entités HTML sont décodées et les lignes parsées (700 g, ½ c. à thé, 4 tasses)
  const rows = page.locator('.ingredient-row')
  await expect(rows).toHaveCount(5)
  await expect(rows.nth(0).getByLabel('Quantité')).toHaveValue('700')
  await expect(rows.nth(0).getByLabel('Nom')).toHaveValue('porc haché maigre')
  await expect(rows.nth(2).getByLabel('Quantité')).toHaveValue('0.5')
  await expect(rows.nth(3).getByLabel('Unité')).toHaveValue('cup')

  // Tags proposés depuis keywords
  await expect(page.getByRole('button', { name: 'Retirer le tag mijoteuse' })).toBeVisible()

  // L'utilisateur corrige la ligne douteuse puis enregistre
  await rows.nth(4).getByLabel('Quantité').fill('2')
  await rows.nth(4).getByLabel('Unité').selectOption('tbsp')
  await page.getByTestId('recipe-submit').click()

  // Fiche : recette sauvegardée avec attribution de source
  await expect(page.getByTestId('recipe-title')).toHaveText('Ragoût de boulettes de grand-maman')
  await expect(page.getByText('Source :')).toBeVisible()
  await expect(page.getByRole('link', { name: /recette-fixture\.html/ })).toBeVisible()
})

test('import par URL : adresse sans recette → erreur claire, rien de sauvegardé', async ({ page, baseURL }) => {
  await registerFreshUser(page)

  await page.goto('/recettes/importer')
  // Attendre l'hydratation : remplir trop tôt laisse v-model réinitialiser le champ.
  await page.waitForLoadState('networkidle')
  await page.getByTestId('import-url').fill(`${baseURL}/login`)
  await page.getByTestId('extract-url').click()

  await expect(page.getByRole('alert')).toContainText('Aucune recette détectée')

  await page.goto('/recettes')
  await expect(page.getByTestId('empty-collection')).toBeVisible()
})
