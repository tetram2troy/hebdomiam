import { expect, test, type Page } from '@playwright/test'
import { loginAsAdmin, registerFreshUser } from './helpers'

// Navigation (docs/design-system.md §8.9) : barre d'onglets basse sous
// 1024 px, navigation haute au-delà. Sur mobile, l'en-tête se réduit au
// wordmark ; langue, déconnexion et administration passent par Réglages.

// IP distincte par fichier de test (rate limiting applicatif : 5/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.81' } })

/** Aucun défilement horizontal de la page (l'ancien en-tête débordait). */
async function expectNoHorizontalOverflow(page: Page) {
  const { scrollWidth, clientWidth } = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    clientWidth: document.documentElement.clientWidth,
  }))
  expect(scrollWidth).toBeLessThanOrEqual(clientWidth)
}

test.describe('mobile (375 px)', () => {
  test.use({ viewport: { width: 375, height: 812 } })

  test('barre d\'onglets basse : quatre destinations, en-tête sur une ligne, rien ne déborde', async ({ page }) => {
    await registerFreshUser(page, 'Marie-Hélène')

    // En-tête : le wordmark seul — plus de liens, de bascule de langue ni de
    // bouton Planifier empilés sur trois rangées (~205 px auparavant).
    const header = (await page.locator('header.topnav').boundingBox())!
    expect(header.height).toBeLessThanOrEqual(64)
    await expect(page.getByTestId('nav-plan')).toBeHidden()
    await expect(page.getByTestId('lang-switch')).toBeHidden()
    await expectNoHorizontalOverflow(page)

    // Barre d'onglets : collée au bas de l'écran, quatre cibles ≥ 48 px.
    const tabbar = page.getByTestId('tabbar')
    await expect(tabbar).toBeVisible()
    const tabs = tabbar.getByRole('link')
    await expect(tabs).toHaveCount(4)
    await expect(tabs).toHaveText(['Accueil', 'Recettes', 'Épicerie', 'Réglages'])
    const bar = (await tabbar.boundingBox())!
    expect(Math.round(bar.y + bar.height)).toBe(812)
    for (const tab of await tabs.all()) {
      expect((await tab.boundingBox())!.height).toBeGreaterThanOrEqual(48)
    }

    // Le CTA « Planifier » vit dans l'accueil, pas dans la barre.
    await expect(page.getByTestId('tab-home')).toHaveAttribute('aria-current', 'page')
    await expect(page.getByTestId('start-planning')).toBeVisible()

    // Chaque onglet mène à sa section et se marque actif.
    await page.getByTestId('tab-recipes').click()
    await expect(page).toHaveURL(/\/recettes$/)
    await expect(page.getByTestId('tab-recipes')).toHaveAttribute('aria-current', 'page')
    await expect(page.getByTestId('tab-home')).not.toHaveAttribute('aria-current', 'page')

    // Une sous-page garde sa section active.
    await page.getByTestId('new-recipe').click()
    await expect(page).toHaveURL(/\/recettes\/nouvelle$/)
    await expect(page.getByTestId('tab-recipes')).toHaveAttribute('aria-current', 'page')

    await page.getByTestId('tab-grocery').click()
    await expect(page).toHaveURL(/\/epicerie$/)
    await expect(page.getByTestId('tab-grocery')).toHaveAttribute('aria-current', 'page')

    await page.getByTestId('tab-settings').click()
    await expect(page).toHaveURL(/\/reglages$/)
    await expect(page.getByTestId('tab-settings')).toHaveAttribute('aria-current', 'page')
    await expect(page.getByTestId('profile-name')).toHaveText('Marie-Hélène')

    // Page longue : la barre reste au bas de l'écran pendant le défilement.
    await page.evaluate(() => window.scrollTo(0, 600))
    await expect.poll(async () => {
      const box = (await tabbar.boundingBox())!
      return Math.round(box.y + box.height)
    }).toBe(812)

    // Un compte normal n'a pas d'entrée Administration.
    await expect(page.getByTestId('settings-admin')).toHaveCount(0)

    // Petit téléphone : toujours rien qui déborde.
    await page.setViewportSize({ width: 320, height: 640 })
    await expectNoHorizontalOverflow(page)
    await page.setViewportSize({ width: 375, height: 812 })

    // La déconnexion passe par Réglages ; déconnecté, plus de barre d'onglets
    // et la bascule de langue revient dans l'en-tête.
    await page.getByRole('button', { name: 'Déconnexion' }).click()
    await expect(page).toHaveURL(/\/login$/)
    await expect(tabbar).toHaveCount(0)
    await expect(page.getByTestId('lang-switch')).toBeVisible()
  })

  test('admin : l\'administration s\'ouvre depuis Réglages, sans débordement', async ({ page }) => {
    await loginAsAdmin(page)

    // Avec le lien Administration en plus, l'ancien en-tête faisait défiler
    // toute la page de côté (460 px de large pour 375).
    await expectNoHorizontalOverflow(page)
    await expect(page.getByTestId('nav-admin')).toBeHidden()

    await page.getByTestId('tab-settings').click()
    await page.getByTestId('settings-admin-link').click()
    await expect(page).toHaveURL(/\/admin$/)
    await expect(page.getByTestId('admin-user-row').first()).toBeVisible()
    // L'administration appartient à la section Réglages.
    await expect(page.getByTestId('tab-settings')).toHaveAttribute('aria-current', 'page')
    await expectNoHorizontalOverflow(page)
  })
})

test('desktop (1280 px) : navigation haute, pas de barre d\'onglets', async ({ page }) => {
  await registerFreshUser(page)

  await expect(page.getByTestId('tabbar')).toBeHidden()
  await expect(page.getByTestId('nav-plan')).toBeVisible()
  await expect(page.getByTestId('nav-settings')).toBeVisible()
  await expect(page.getByTestId('lang-switch')).toBeVisible()
})
