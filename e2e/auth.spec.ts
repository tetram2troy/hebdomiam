import { expect, test } from '@playwright/test'

// Parcours critique M0 : inscription → déconnexion → connexion.
const email = `m0-${Date.now()}@example.com`
const password = 'motdepasse-tres-solide-123'
const name = 'Testeur M0'

test('inscription, déconnexion puis reconnexion', async ({ page }) => {
  await page.goto('/register')
  // Attendre l'hydratation : remplir trop tôt laisse v-model réinitialiser les champs.
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Nom').fill(name)
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(password)
  await page.getByRole('button', { name: "S'inscrire" }).click()
  await expect(page.getByTestId('welcome')).toContainText(name)

  await page.getByRole('button', { name: 'Déconnexion' }).click()
  await expect(page).toHaveURL(/\/login$/)

  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill(password)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByTestId('welcome')).toContainText(name)
})

test('refuse un mauvais mot de passe', async ({ page }) => {
  await page.goto('/login')
  await page.waitForLoadState('networkidle')
  await page.getByLabel('Courriel').fill(email)
  await page.getByLabel('Mot de passe').fill('mauvais-mot-de-passe-999')
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByRole('alert')).toContainText('invalide')
  await expect(page).toHaveURL(/\/login$/)
})
