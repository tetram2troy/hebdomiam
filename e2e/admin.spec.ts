import { expect, test } from '@playwright/test'
import { ADMIN_EMAIL, FRESH_PASSWORD, loginAsAdmin, registerFreshUser } from './helpers'

// Administration des comptes : accès réservé au rôle admin, création par
// invitation (URL copiable), désactivation effective sur une session active,
// garde-fous « self » et suppression avec confirmation.

// IP distincte par fichier de test (rate limiting applicatif : 5/min/IP) ;
// le contexte de l'invitée utilise .62.
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.61' } })

test('un utilisateur normal ne voit ni n\'atteint l\'administration', async ({ page }) => {
  await registerFreshUser(page, 'Sans droits')

  // Pas de lien dans la nav
  await expect(page.getByTestId('nav-admin')).toHaveCount(0)

  // La page redirige (garde d'affichage côté client)
  await page.goto('/admin')
  await page.waitForLoadState('networkidle')
  await expect(page).not.toHaveURL(/\/admin/)

  // La vraie défense est côté serveur : requireAdminSession relit le rôle en
  // base. Fetch depuis le navigateur : cookies de session natifs.
  const status = await page.evaluate(async () => (await fetch('/api/admin/users')).status)
  expect(status).toBe(403)
})

test('cycle admin : invitation, activation, désactivation effective, suppression', async ({ page, browser }) => {
  test.setTimeout(120_000) // parcours long : deux navigateurs, plusieurs connexions

  await loginAsAdmin(page)
  await expect(page.getByTestId('nav-admin')).toBeVisible()
  await page.getByTestId('nav-admin').click()
  await expect(page).toHaveURL(/\/admin$/)
  await page.waitForLoadState('networkidle') // hydratation avant de cliquer

  // ── Aucune action possible sur sa propre ligne ────────────────────────────
  const myRow = page.getByTestId('admin-user-row').filter({ hasText: ADMIN_EMAIL })
  await expect(myRow).toHaveCount(1)
  await expect(myRow.getByTestId('admin-row-delete')).toHaveCount(0)

  // …et le serveur refuse aussi (garde « self », pas seulement l'UI).
  // Fetch depuis le navigateur : cookies de session natifs.
  const selfPatchStatus = await page.evaluate(async (adminEmail) => {
    const accounts = await (await fetch('/api/admin/users')).json() as Array<{ id: string, email: string }>
    const myId = accounts.find(account => account.email === adminEmail)!.id
    const response = await fetch(`/api/admin/users/${myId}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ disabled: true }),
    })
    return response.status
  }, ADMIN_EMAIL)
  expect(selfPatchStatus).toBe(400)

  // ── Création d'un compte → lien d'invitation affiché et copiable ──────────
  const email = `admin-invitee-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.getByTestId('admin-create-name').fill('Invitée')
  await page.getByTestId('admin-create-email').fill(email)
  await page.getByTestId('admin-create-submit').click()
  await expect(page.getByTestId('admin-invite')).toBeVisible()
  const inviteUrl = await page.getByTestId('admin-invite-url').inputValue()
  expect(inviteUrl).toContain('/invitation?token=')

  // Verrou visuel : le champ du lien s'étire réellement dans le panneau
  // (régression déjà vécue sur les champs URL — voir commit 8c0ce81).
  const urlBox = await page.getByTestId('admin-invite-url').boundingBox()
  expect(urlBox!.width).toBeGreaterThanOrEqual(400)

  const inviteeRow = page.getByTestId('admin-user-row').filter({ hasText: email })
  await expect(inviteeRow.getByText('invitation en attente')).toBeVisible()

  // ── L'invitée active son compte dans son propre navigateur ────────────────
  const inviteeCtx = await browser.newContext({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.62' } })
  const inviteePage = await inviteeCtx.newPage()
  await inviteePage.goto(inviteUrl)
  await inviteePage.waitForLoadState('networkidle') // hydratation avant de remplir
  await inviteePage.getByTestId('invite-password').fill(FRESH_PASSWORD)
  await inviteePage.getByTestId('invite-submit').click()
  await expect(inviteePage.getByTestId('invite-done')).toBeVisible()

  await inviteePage.goto('/login')
  await inviteePage.waitForLoadState('networkidle')
  await inviteePage.getByLabel('Courriel').fill(email)
  await inviteePage.getByLabel('Mot de passe').fill(FRESH_PASSWORD)
  await inviteePage.getByRole('button', { name: 'Se connecter' }).click()
  await expect(inviteePage.getByTestId('welcome')).toContainText('Invitée')

  // ── Désactivation : la session active est coupée immédiatement ────────────
  await inviteeRow.getByTestId('admin-row-disable').click()
  await expect(page.getByTestId('admin-notice')).toBeVisible()

  // L'API répond 401 et purge le cookie de session (middleware account-status).
  // Fetch depuis le navigateur : le Set-Cookie de purge s'applique au contexte.
  const blockedStatus = await inviteePage.evaluate(async () => (await fetch('/api/recipes')).status)
  expect(blockedStatus).toBe(401)
  await inviteePage.goto('/')
  await inviteePage.waitForLoadState('networkidle')
  await expect(inviteePage.getByTestId('welcome')).toHaveCount(0)

  // Reconnexion refusée avec un message explicite (mot de passe pourtant bon)
  await inviteePage.goto('/login')
  await inviteePage.waitForLoadState('networkidle')
  await inviteePage.getByLabel('Courriel').fill(email)
  await inviteePage.getByLabel('Mot de passe').fill(FRESH_PASSWORD)
  await inviteePage.getByRole('button', { name: 'Se connecter' }).click()
  await expect(inviteePage.getByRole('alert')).toContainText('désactivé')

  // ── Réactivation : la connexion repasse ───────────────────────────────────
  await inviteeRow.getByTestId('admin-row-disable').click()
  await expect(page.getByTestId('admin-notice')).toBeVisible()

  await inviteePage.getByLabel('Mot de passe').fill(FRESH_PASSWORD)
  await inviteePage.getByRole('button', { name: 'Se connecter' }).click()
  await expect(inviteePage.getByTestId('welcome')).toContainText('Invitée')

  // ── Suppression avec confirmation ─────────────────────────────────────────
  await inviteeRow.getByTestId('admin-row-delete').click()
  await expect(page.getByTestId('confirm-dialog')).toBeVisible()
  await page.getByTestId('confirm-dialog-confirm').click()
  await expect(page.getByTestId('admin-notice')).toBeVisible()
  await expect(inviteeRow).toHaveCount(0)

  // Le compte n'existe plus : identifiants refusés sans fuite d'information
  await inviteePage.goto('/login')
  await inviteePage.waitForLoadState('networkidle')
  await inviteePage.getByLabel('Courriel').fill(email)
  await inviteePage.getByLabel('Mot de passe').fill(FRESH_PASSWORD)
  await inviteePage.getByRole('button', { name: 'Se connecter' }).click()
  await expect(inviteePage.getByRole('alert')).toContainText('invalide')

  await inviteeCtx.close()
})
