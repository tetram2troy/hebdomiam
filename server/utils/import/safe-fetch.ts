import { lookup } from 'node:dns/promises'
import { isIP } from 'node:net'

// Récupération d'une page externe durcie contre le SSRF (PRD §8) :
// https seulement, IP privées/loopback/link-local bloquées (y compris via
// redirections), timeout et taille de réponse plafonnés.
// Limite connue (V1) : la validation DNS précède le fetch (fenêtre de
// rebinding théorique) — accepté, documenté ici.

const MAX_BYTES = 2 * 1024 * 1024
const TIMEOUT_MS = 10_000
const MAX_REDIRECTS = 3
const USER_AGENT = 'Mozilla/5.0 (compatible; HebdomiamBot/1.0; import de recettes personnel)'

export type SafeFetchResult
  = | { ok: true, html: string, finalUrl: string }
    | { ok: false, reason: 'unsupported' | 'blocked' | 'unreachable' | 'tooLarge' }

/** Les tests E2E servent une page-fixture locale : bascule explicite, jamais active par défaut. */
function allowPrivate(): boolean {
  return process.env.NUXT_IMPORT_ALLOW_PRIVATE === '1'
}

export function isPrivateIp(ip: string): boolean {
  if (ip.includes(':')) {
    const lower = ip.toLowerCase()
    const mapped = lower.match(/^::ffff:(\d+\.\d+\.\d+\.\d+)$/)
    if (mapped) return isPrivateIp(mapped[1]!)
    if (lower === '::1' || lower === '::') return true
    // fc00::/7 (ULA) et fe80::/10 (link-local)
    return /^f[cd]/.test(lower) || /^fe[89ab]/.test(lower)
  }
  const parts = ip.split('.').map(Number)
  if (parts.length !== 4 || parts.some(n => Number.isNaN(n) || n > 255)) return true
  const [a, b] = parts as [number, number, number, number]
  return a === 0 || a === 10 || a === 127
    || (a === 100 && b >= 64 && b <= 127) // CGNAT
    || (a === 169 && b === 254)
    || (a === 172 && b >= 16 && b <= 31)
    || (a === 192 && b === 168)
    || a >= 224
}

async function isUrlAllowed(url: URL): Promise<boolean> {
  if (allowPrivate()) return url.protocol === 'https:' || url.protocol === 'http:'
  if (url.protocol !== 'https:') return false
  const host = url.hostname
  if (isIP(host)) return !isPrivateIp(host)
  try {
    const addresses = await lookup(host, { all: true })
    return addresses.length > 0 && addresses.every(addr => !isPrivateIp(addr.address))
  }
  catch {
    return false
  }
}

async function readCapped(response: Response): Promise<string | null> {
  const reader = response.body?.getReader()
  if (!reader) return null
  const chunks: Uint8Array[] = []
  let total = 0
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    total += value.length
    if (total > MAX_BYTES) {
      await reader.cancel()
      return null
    }
    chunks.push(value)
  }
  return Buffer.concat(chunks).toString('utf8')
}

export async function fetchExternalPage(rawUrl: string): Promise<SafeFetchResult> {
  let url: URL
  try {
    url = new URL(rawUrl)
  }
  catch {
    return { ok: false, reason: 'unsupported' }
  }

  for (let redirects = 0; redirects <= MAX_REDIRECTS; redirects++) {
    if (!(await isUrlAllowed(url))) {
      return { ok: false, reason: 'blocked' }
    }

    let response: Response
    try {
      response = await fetch(url, {
        redirect: 'manual',
        signal: AbortSignal.timeout(TIMEOUT_MS),
        headers: {
          'user-agent': USER_AGENT,
          'accept': 'text/html,application/xhtml+xml',
          'accept-language': 'fr-CA,fr;q=0.9,en;q=0.8',
        },
      })
    }
    catch {
      return { ok: false, reason: 'unreachable' }
    }

    if (response.status >= 300 && response.status < 400) {
      const location = response.headers.get('location')
      if (!location || redirects === MAX_REDIRECTS) return { ok: false, reason: 'unreachable' }
      url = new URL(location, url)
      continue
    }

    if (!response.ok) return { ok: false, reason: 'unreachable' }

    const contentType = response.headers.get('content-type') ?? ''
    if (contentType && !/text\/html|application\/xhtml|text\/plain/.test(contentType)) {
      return { ok: false, reason: 'unsupported' }
    }

    const html = await readCapped(response)
    if (html === null) return { ok: false, reason: 'tooLarge' }
    return { ok: true, html, finalUrl: url.toString() }
  }

  return { ok: false, reason: 'unreachable' }
}
