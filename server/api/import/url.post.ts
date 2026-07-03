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

  const page = await fetchExternalPage(url)
  if (!page.ok) {
    throw createError({ statusCode: 422, message: 'errors.importUnreachable' })
  }

  // 1. Gratuit et instantané : schema.org/Recipe (JSON-LD).
  const draft = extractRecipeFromHtml(page.html, url)
  if (draft) {
    return { draft }
  }

  // 2. Repli IA — seulement si configuré (D1).
  if (!isClaudeConfigured()) {
    throw createError({ statusCode: 422, message: 'errors.importFailed' })
  }
  const result = await extractRecipeWithClaude({ html: page.html, sourceUrl: url })
  if (!result.ok) {
    throw createError({
      statusCode: result.reason === 'notFound' ? 422 : 502,
      message: result.reason === 'notFound' ? 'errors.importFailed' : 'errors.importProviderError',
    })
  }
  return { draft: result.draft }
})
