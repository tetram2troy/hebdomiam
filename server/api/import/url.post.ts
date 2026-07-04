import { importUrlSchema } from '#shared/schemas/import'
import { extractRecipeWithClaude, isClaudeConfigured } from '../../utils/import/claude-provider'
import { fetchExternalPage } from '../../utils/import/safe-fetch'
import { extractRecipeFromHtml } from '../../utils/import/schema-org'

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  // Quota par utilisateur : l'import déclenche des fetchs sortants et,
  // en repli, des appels IA facturés (PRD D9).
  assertRateLimit(event, `import:url:${user.id}`, { max: 10, windowMs: 60_000 })

  const { url } = await readValidatedBody(event, data => importUrlSchema.parse(data))

  // Le domaine suffit à l'audit — l'URL complète peut être longue et parlante.
  const host = new URL(url).hostname

  const page = await fetchExternalPage(url)
  if (!page.ok) {
    logWarn('import.url_unreachable', { userId: user.id, host, reason: page.reason })
    throw createError({ statusCode: 422, message: 'errors.importUnreachable' })
  }

  // 1. Gratuit et instantané : schema.org/Recipe (JSON-LD).
  const draft = extractRecipeFromHtml(page.html, url)
  if (draft) {
    logEvent('import.url', { userId: user.id, host, provider: 'schema-org' })
    return { draft }
  }

  // 2. Repli IA — seulement si configuré (D1).
  if (!isClaudeConfigured()) {
    logWarn('import.url_failed', { userId: user.id, host, reason: 'noRecipeNoProvider' })
    throw createError({ statusCode: 422, message: 'errors.importFailed' })
  }
  const result = await extractRecipeWithClaude({ html: page.html, sourceUrl: url })
  if (!result.ok) {
    logWarn('import.url_failed', { userId: user.id, host, reason: result.reason })
    throw createError({
      statusCode: result.reason === 'notFound' ? 422 : 502,
      message: result.reason === 'notFound' ? 'errors.importFailed' : 'errors.importProviderError',
    })
  }
  // Repli facturé (PRD D9) : suivre la fréquence d'usage du fournisseur.
  logEvent('import.url', { userId: user.id, host, provider: 'claude' })
  return { draft: result.draft }
})
