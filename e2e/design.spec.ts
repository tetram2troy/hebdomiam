import { expect, test, type Locator, type Page } from '@playwright/test'

// Conformité au design system « Primeur » (docs/design-system.md, app/assets/css/tokens.css) :
// tokens réellement appliqués, fontes chargées, thèmes clair/sombre, focus visible,
// cibles tactiles. Ces tests protègent le design contre les régressions CSS.

// IP distincte par fichier de test (rate limiting d'inscription : 5/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.21' } })

const PRIMARY_LIGHT = 'rgb(30, 122, 61)' // --md-sys-color-primary (#1E7A3D)
const SURFACE_LIGHT = 'rgb(247, 250, 244)' // --md-sys-color-surface (#F7FAF4)
const PRIMARY_DARK = 'rgb(82, 194, 115)' // #52C273
const SURFACE_DARK = 'rgb(16, 24, 18)' // #101812

function style(locator: Locator, property: string) {
  return locator.evaluate(
    (el, prop) => getComputedStyle(el).getPropertyValue(prop),
    property,
  )
}

async function registerFreshUser(page: Page) {
  const email = `design-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill('Design QA')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mot-de-passe-tres-long-123')
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
}

test.describe('thème clair', () => {
  test.use({ colorScheme: 'light' })

  test('tokens appliqués : surface, action primaire, pastilles', async ({ page }) => {
    await page.goto('/login')

    expect(await style(page.locator('body'), 'background-color')).toBe(SURFACE_LIGHT)

    const submit = page.getByRole('button', { name: 'Se connecter' })
    expect(await style(submit, 'background-color')).toBe(PRIMARY_LIGHT)
    expect(await style(submit, 'border-radius')).toBe('9999px')
    expect(await style(submit, 'color')).toBe('rgb(255, 255, 255)')
  })

  test('typographie : Bricolage Grotesque (titres) et Figtree (texte) chargées et actives', async ({ page }) => {
    await page.goto('/login')
    await page.evaluate(() => document.fonts.ready)

    expect(await page.evaluate(() => document.fonts.check('700 24px "Bricolage Grotesque"'))).toBe(true)
    expect(await page.evaluate(() => document.fonts.check('400 16px "Figtree"'))).toBe(true)

    expect(await style(page.locator('h1').first(), 'font-family')).toContain('Bricolage Grotesque')
    expect(await style(page.locator('body'), 'font-family')).toContain('Figtree')
  })

  test('focus visible au clavier : anneau primaire, jamais supprimé', async ({ page }) => {
    await page.goto('/login')
    const email = page.getByLabel('Courriel')
    await email.focus()
    const shadow = await style(email, 'box-shadow')
    expect(shadow).toContain('30, 122, 61')
  })

  test('cibles tactiles : boutons et chips-filtres ≥ 44 px', async ({ page }) => {
    await registerFreshUser(page)
    await page.goto('/recettes')

    const cta = page.getByTestId('new-recipe')
    const box = await cta.boundingBox()
    expect(box!.height).toBeGreaterThanOrEqual(44)

    const search = page.getByLabel('Chercher une recette')
    const searchBox = await search.boundingBox()
    expect(searchBox!.height).toBeGreaterThanOrEqual(44)
  })

  test('cartes recette : coins arrondis « large » et placeholder illustré tokenisé', async ({ page }) => {
    await registerFreshUser(page)
    await page.goto('/recettes/nouvelle')
    await page.getByLabel('Titre *').fill('Soupe test design')
    await page.getByLabel('Portions de base *').fill('2')
    const row = page.locator('.ingredient-row').first()
    await row.getByLabel('Quantité').fill('1')
    await row.getByLabel('Nom').fill('eau')
    await page.getByLabel('Étape 1', { exact: true }).fill('Faire bouillir.')
    const tagInput = page.getByLabel('Tags')
    await tagInput.fill('soupe')
    await tagInput.press('Enter')
    await page.getByTestId('recipe-submit').click()
    await expect(page.getByTestId('recipe-title')).toBeVisible()

    await page.goto('/recettes')
    const card = page.getByTestId('recipe-card')
    expect(await style(card, 'border-radius')).toBe('16px')
    // Placeholder illustré : fond secondary-container (#DFF0D9)
    const placeholder = card.locator('.ph')
    expect(await style(placeholder, 'background-color')).toBe('rgb(223, 240, 217)')
  })
})

test.describe('thème sombre (parité)', () => {
  test.use({ colorScheme: 'dark' })

  test('tokens sombres appliqués sans bascule manuelle', async ({ page }) => {
    await page.goto('/login')

    expect(await style(page.locator('body'), 'background-color')).toBe(SURFACE_DARK)

    const submit = page.getByRole('button', { name: 'Se connecter' })
    expect(await style(submit, 'background-color')).toBe(PRIMARY_DARK)
    // on-primary sombre : #06301A
    expect(await style(submit, 'color')).toBe('rgb(6, 48, 26)')
  })
})
