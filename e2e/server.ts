import { type ChildProcess, execFileSync, spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { BASE_URL, DB_URL, E2E_PORT } from './env'

// Serveur de la suite e2e : le VRAI binaire de l'API Rust, qui sert aussi le
// front généré (.output/public — `npm run generate` avant la suite).
//
// Deux modes :
// - E2E_API_BIN=<chemin du binaire> : lancé directement (CI, après cargo build) ;
// - sinon : image Docker `api-runtime` construite depuis le Dockerfile (poste
//   de dev sans compilateur Rust). Le conteneur est recréé à chaque exécution :
//   jamais de serveur résiduel avec un vieux build.
//
// Environnement de test commun : IMPORT_ALLOW_PRIVATE=1 autorise la
// page-fixture locale de l'import ; TRUST_XFF=1 laisse chaque spec simuler son
// IP (buckets de rate limiting isolés). Jamais activés hors tests.

const CONTAINER = 'hebdomiam-e2e'
const IMAGE = 'hebdomiam-api:e2e'
const STATIC_DIR = resolve('.output/public')
const SESSION_PASSWORD = process.env.SESSION_PASSWORD ?? 'e2e-session-password-of-at-least-32-chars!!'

function testEnv(databaseUrl: string): Record<string, string> {
  return {
    DATABASE_URL: databaseUrl,
    SESSION_PASSWORD,
    PORT: String(E2E_PORT),
    APP_ENV: 'test',
    IMPORT_ALLOW_PRIVATE: '1',
    TRUST_XFF: '1',
  }
}

async function waitForHealth(timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${BASE_URL}/api/health`)
      if (response.ok) return
    }
    catch {
      // pas encore prêt
    }
    await new Promise(r => setTimeout(r, 500))
  }
  throw new Error(`le serveur e2e ne répond pas sur ${BASE_URL}/api/health`)
}

function docker(args: string[], inherit = false): string {
  return execFileSync('docker', args, { encoding: 'utf8', stdio: inherit ? 'inherit' : ['ignore', 'pipe', 'pipe'] }) ?? ''
}

/** Démarre le serveur ; retourne la fonction d'arrêt. */
export async function startServer(): Promise<() => Promise<void>> {
  if (!DB_URL) throw new Error('DATABASE_URL manquant : charge le .env avant npm run test:e2e')
  if (!existsSync(resolve(STATIC_DIR, 'index.html'))) {
    throw new Error('front absent : lance `npm run generate` avant la suite e2e')
  }

  const bin = process.env.E2E_API_BIN
  if (bin) {
    const env = { ...process.env, ...testEnv(DB_URL), STATIC_DIR }
    execFileSync(bin, ['migrate'], { env, stdio: 'inherit' })
    const child: ChildProcess = spawn(bin, ['serve'], { env, stdio: ['ignore', 'ignore', 'inherit'] })
    await waitForHealth(30_000)
    return async () => {
      child.kill('SIGTERM')
    }
  }

  // Mode Docker : la base de l'hôte est joignable via host.docker.internal.
  const url = new URL(DB_URL)
  if (['localhost', '127.0.0.1', '::1'].includes(url.hostname)) url.hostname = 'host.docker.internal'
  const env = testEnv(url.toString())
  const envArgs = Object.entries(env).flatMap(([k, v]) => ['-e', `${k}=${v}`])
  const hostArgs = ['--add-host', 'host.docker.internal:host-gateway']

  try {
    docker(['rm', '-f', CONTAINER])
  }
  catch {
    // aucun conteneur résiduel
  }
  docker(['build', '--target', 'api-runtime', '-t', IMAGE, '.'], true)
  docker(['run', '--rm', ...hostArgs, ...envArgs, IMAGE, 'migrate'], true)
  docker([
    'run', '-d', '--name', CONTAINER, ...hostArgs, ...envArgs,
    '-p', `127.0.0.1:${E2E_PORT}:${E2E_PORT}`,
    '-v', `${STATIC_DIR}:/app/public:ro`,
    IMAGE,
  ])
  try {
    await waitForHealth(60_000)
  }
  catch (error) {
    console.error(docker(['logs', CONTAINER]))
    throw error
  }
  return async () => {
    try {
      docker(['rm', '-f', CONTAINER])
    }
    catch {
      // déjà arrêté
    }
  }
}
