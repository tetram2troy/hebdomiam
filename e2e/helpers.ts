import { expect, request, type APIResponse, type Page } from '@playwright/test'

// L'inscription publique est fermée : les comptes de test sont créés via
// l'API admin (compte seedé par global-setup) puis l'invitation est acceptée
// par API. Même contrat que l'ancien helper local des specs : retourne le
// courriel et laisse `page` connectée sur l'accueil.

export const ADMIN_EMAIL = 'admin-e2e@example.com'
export const ADMIN_PASSWORD = 'mot-de-passe-admin-e2e-123'
export const FRESH_PASSWORD = 'mot-de-passe-tres-long-123'

const BASE_URL = 'http://127.0.0.1:3000'

// Pool d'IP dédié aux appels API du helper (198.51.x.y, tirée au hasard par
// appel) : le budget de rate limiting de l'IP du fichier de spec (203.0.113.x)
// reste intact, et deux workers Playwright parallèles ne se marchent pas
// dessus (un compteur par module se réinitialise à chaque worker — vécu).
// Une collision improbable coûte 4 logins/min sur l'IP, sous la limite de 5.
function helperIp(): string {
  const octet = () => Math.floor(Math.random() * 250) + 1
  return `198.51.${octet()}.${octet()}`
}

/** Le cookie de session sort du build de prod avec l'attribut Secure : le jar
    du contexte API le refuse en http, on le transporte donc explicitement.
    (Le navigateur, lui, traite 127.0.0.1 comme origine de confiance.) */
function sessionCookie(response: APIResponse): string {
  const setCookie = response.headersArray()
    .find(header => header.name.toLowerCase() === 'set-cookie' && header.value.startsWith('nuxt-session='))
  const value = /^nuxt-session=([^;]+)/.exec(setCookie?.value ?? '')?.[1]
  expect(value, 'cookie de session présent dans la réponse').toBeTruthy()
  return value!
}

/** Crée un compte frais (API admin + invitation) et connecte `page`. */
export async function registerFreshUser(page: Page, name = 'Testeuse'): Promise<string> {
  const email = `e2e-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`

  const api = await request.newContext({
    baseURL: BASE_URL,
    extraHTTPHeaders: { 'x-forwarded-for': helperIp() },
  })

  // 1. L'admin seedé crée le compte et récupère le lien d'invitation.
  const adminLogin = await api.post('/api/auth/login', {
    data: { email: ADMIN_EMAIL, password: ADMIN_PASSWORD },
  })
  expect(adminLogin.ok(), 'connexion admin e2e (global-setup exécuté ?)').toBeTruthy()
  const created = await api.post('/api/admin/users', {
    data: { email, name },
    headers: { cookie: `nuxt-session=${sessionCookie(adminLogin)}` },
  })
  expect(created.status(), 'création du compte invité').toBe(201)
  const { inviteUrl } = await created.json() as { inviteUrl: string }
  const token = new URL(inviteUrl).searchParams.get('token')!

  // 2. L'invité choisit son mot de passe puis se connecte.
  const accepted = await api.post('/api/auth/invitation', {
    data: { token, password: FRESH_PASSWORD },
  })
  expect(accepted.ok(), 'acceptation de l\'invitation').toBeTruthy()
  const login = await api.post('/api/auth/login', {
    data: { email, password: FRESH_PASSWORD },
  })
  expect(login.ok(), 'connexion du compte frais').toBeTruthy()

  // 3. Le cookie de session passe au navigateur — sans attribut Secure, pour
  //    que Chromium l'envoie sur http://127.0.0.1 sans condition.
  await page.context().addCookies([{
    name: 'nuxt-session',
    value: sessionCookie(login),
    domain: '127.0.0.1',
    path: '/',
  }])
  await api.dispose()

  await page.goto('/')
  await page.waitForLoadState('networkidle') // hydratation
  await expect(page.getByTestId('welcome')).toBeVisible()
  return email
}

/** Connecte `page` avec le compte admin seedé (via l'UI de connexion). */
export async function loginAsAdmin(page: Page): Promise<void> {
  await page.goto('/login')
  await page.waitForLoadState('networkidle') // hydratation avant de remplir
  await page.getByLabel('Courriel').fill(ADMIN_EMAIL)
  await page.getByLabel('Mot de passe').fill(ADMIN_PASSWORD)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByTestId('welcome')).toBeVisible()
}
