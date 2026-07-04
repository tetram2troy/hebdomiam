import sharp from 'sharp'
import { extractRecipeWithClaude, isClaudeConfigured } from '../../utils/import/claude-provider'

const MAX_FILES = 3
const MAX_BYTES = 10 * 1024 * 1024

/** Vérification du type réel par nombres magiques (PRD §8) — jamais le nom du fichier. */
function sniffImageType(data: Buffer): 'image/jpeg' | 'image/png' | 'image/webp' | null {
  if (data.length > 3 && data[0] === 0xFF && data[1] === 0xD8 && data[2] === 0xFF) return 'image/jpeg'
  if (data.length > 8 && data[0] === 0x89 && data[1] === 0x50 && data[2] === 0x4E && data[3] === 0x47) return 'image/png'
  if (data.length > 12 && data.toString('ascii', 0, 4) === 'RIFF' && data.toString('ascii', 8, 12) === 'WEBP') return 'image/webp'
  return null
}

export default defineEventHandler(async (event) => {
  const { user } = await requireUserSession(event)
  assertRateLimit(event, `import:photo:${user.id}`, { max: 5, windowMs: 60_000 })

  if (!isClaudeConfigured()) {
    throw createError({ statusCode: 503, message: 'errors.importUnavailable' })
  }

  const parts = await readMultipartFormData(event)
  const photos = (parts ?? []).filter(part => part.name === 'photos' && part.data.length > 0)
  if (photos.length === 0 || photos.length > MAX_FILES) {
    throw createError({ statusCode: 400, message: 'errors.invalidImage' })
  }

  const images = []
  for (const photo of photos) {
    if (photo.data.length > MAX_BYTES) {
      throw createError({ statusCode: 413, message: 'errors.fileTooLarge' })
    }
    if (!sniffImageType(photo.data)) {
      throw createError({ statusCode: 415, message: 'errors.invalidImage' })
    }
    // Ré-encodage systématique : supprime EXIF/métadonnées et neutralise les
    // charges utiles cachées ; borne aussi la taille envoyée au fournisseur.
    try {
      const reencoded = await sharp(photo.data)
        .rotate() // applique l'orientation EXIF avant de la jeter
        .resize({ width: 1600, height: 1600, fit: 'inside', withoutEnlargement: true })
        .jpeg({ quality: 80 })
        .toBuffer()
      images.push({ mediaType: 'image/jpeg' as const, dataBase64: reencoded.toString('base64') })
    }
    catch {
      throw createError({ statusCode: 415, message: 'errors.invalidImage' })
    }
  }

  const result = await extractRecipeWithClaude({ images })
  if (!result.ok) {
    logWarn('import.photo_failed', { userId: user.id, photos: images.length, reason: result.reason })
    throw createError({
      statusCode: result.reason === 'notFound' ? 422 : 502,
      message: result.reason === 'notFound' ? 'errors.importFailed' : 'errors.importProviderError',
    })
  }
  // Chaque appel vision est facturé (PRD D9) : suivre le volume par utilisateur.
  logEvent('import.photo', { userId: user.id, photos: images.length })
  return { draft: result.draft }
})
