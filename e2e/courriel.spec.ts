import { createHash, randomBytes } from 'node:crypto'
import { expect, test, type Page } from '@playwright/test'
import pg from 'pg'

// Parcours par courriel (M5) : réinitialisation de mot de passe et vérification
// d'adresse. Seule l'empreinte SHA-256 des jetons est en base : le test insère
// donc lui-même un jeton dont il connaît la valeur brute — exactement ce que
// produit createEmailToken côté serveur.

// IP distincte par fichier de test (rate limiting : 5 logins/min/IP).
test.use({ extraHTTPHeaders: { 'x-forwarded-for': '203.0.113.53' } })

const PASSWORD = 'mot-de-passe-tres-long-123'
const DB_URL = process.env.NUXT_DATABASE_URL
  ?? 'postgresql://hebdomiam:hebdomiam@localhost:5432/hebdomiam'

async function insertEmailToken(email: string, type: 'verify' | 'reset'): Promise<string> {
  const raw = randomBytes(24).toString('hex')
  const client = new pg.Client({ connectionString: DB_URL })
  await client.connect()
  try {
    const { rows } = await client.query('SELECT id FROM users WHERE email = $1', [email])
    if (!rows[0]) throw new Error(`utilisateur introuvable : ${email}`)
    await client.query(
      `INSERT INTO email_tokens (user_id, type, token_hash, expires_at)
       VALUES ($1, $2, $3, now() + interval '1 hour')`,
      [rows[0].id, type, createHash('sha256').update(raw).digest('hex')],
    )
  }
  finally {
    await client.end()
  }
  return raw
}

async function registerFreshUser(page: Page) {
  const email = `courriel-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`
  await page.goto('/register')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill('Distraite')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(PASSWORD)
  await page.getByRole('button', { name: 'S\'inscrire' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
  return email
}

test('mot de passe oublié : demande anonyme puis réinitialisation par jeton', async ({ page }) => {
  const email = await registerFreshUser(page)
  await page.getByRole('button', { name: 'Déconnexion' }).click()

  // La page « oublié » répond pareil que le compte existe ou non (anti-énumération)
  await page.getByRole('link', { name: 'Mot de passe oublié ?' }).click()
  // Attendre la navigation : /login a aussi un champ « Courriel », remplir trop
  // tôt écrirait dans le mauvais formulaire.
  await expect(page).toHaveURL(/\/mot-de-passe-oublie$/)
  await page.getByLabel('Courriel').fill(email)
  await page.getByRole('button', { name: 'Envoyer le lien' }).click()
  await expect(page.getByTestId('forgot-sent')).toBeVisible()

  // Suivre le lien du courriel (jeton inséré en base, valeur brute connue du test)
  const token = await insertEmailToken(email, 'reset')
  const newPassword = 'encore-plus-solide-456'
  await page.goto(`/reinitialiser?token=${token}`)
  await page.waitForLoadState('networkidle') // hydratation avant de remplir
  await page.getByLabel('Mot de passe').fill(newPassword)
  await page.getByTestId('reset-submit').click()
  await expect(page.getByTestId('reset-done')).toBeVisible()

  // Usage unique : rejouer le même lien échoue
  await page.goto(`/reinitialiser?token=${token}`)
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Mot de passe').fill('troisieme-mot-de-passe-789')
  await page.getByTestId('reset-submit').click()
  await expect(page.getByRole('alert')).toContainText('invalide')

  // Le nouveau mot de passe connecte
  await page.goto('/login')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(newPassword)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
})

test('vérification du courriel : le lien confirme et les réglages l\'affichent', async ({ page }) => {
  const email = await registerFreshUser(page)

  await page.goto('/reglages')
  await expect(page.getByText('non vérifié')).toBeVisible()

  const token = await insertEmailToken(email, 'verify')
  await page.goto(`/verifier-courriel?token=${token}`)
  await expect(page.getByTestId('verify-done')).toBeVisible()

  await page.goto('/reglages')
  await expect(page.getByText('✓ vérifié')).toBeVisible()
  await expect(page.getByTestId('resend-verify')).toHaveCount(0)
})

test('vérification du courriel : lien falsifié refusé', async ({ page }) => {
  await page.goto(`/verifier-courriel?token=${'f'.repeat(48)}`)
  await expect(page.getByTestId('verify-failed')).toBeVisible()
})
