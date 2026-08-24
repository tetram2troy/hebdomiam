import type { H3Event } from 'h3'
import { getRequestHeader, getRequestIP } from 'h3'

// IP client derrière Cloudflare puis Traefik (docs/HOSTING.md §2.3).
// X-Forwarded-For n'est PAS fiable : getRequestIP en lit l'entrée la plus à
// GAUCHE, fournie par le client lui-même — Cloudflare ne fait qu'ajouter l'IP
// réelle à droite. S'y fier rendrait le rate limiting d'auth contournable par
// simple rotation d'en-tête (et les logs de sécurité usurpables).
// Ordre de résolution :
// 1. cf-connecting-ip — posé par Cloudflare, seul en-tête digne de confiance
//    en production (le DNS wildcard du home lab est toujours proxifié) ;
// 2. X-Forwarded-For si NUXT_TRUST_XFF est défini — les e2e isolent leurs
//    buckets de rate limiting en simulant des IP distinctes
//    (playwright.config.ts) ; jamais défini en production ;
// 3. IP socket — l'appelant direct (Traefik, ou le navigateur en dev).
export function resolveClientIp(
  event: H3Event,
  env: Record<string, string | undefined> = process.env,
): string {
  const cloudflare = getRequestHeader(event, 'cf-connecting-ip')
  if (cloudflare) return cloudflare
  if (env.NUXT_TRUST_XFF) return getRequestIP(event, { xForwardedFor: true }) ?? 'unknown'
  return getRequestIP(event) ?? 'unknown'
}
