import Anthropic from '@anthropic-ai/sdk'
import { zodOutputFormat } from '@anthropic-ai/sdk/helpers/zod'
import { z } from 'zod'
import { AISLES, UNITS } from '#shared/schemas/recipe'
import type { ImportDraft } from '#shared/types/import'
import { guessAisle } from './ingredient-line'

// Fournisseur d'extraction « claude » (PRD §7.4) : repli pour les URLs sans
// schema.org et voie principale pour les photos. La clé API ne quitte jamais
// le serveur ; la sortie est traitée comme non fiable (schéma + relecture).

const DEFAULT_MODEL = 'claude-opus-4-8'

/** Schéma imposé au modèle via les sorties structurées — le SDK valide. */
const extractionSchema = z.object({
  found: z.boolean().describe('false si le contenu ne contient aucune recette'),
  title: z.string().nullable(),
  description: z.string().nullable(),
  baseServings: z.number().int().nullable().describe('nombre de portions de la recette'),
  prepMinutes: z.number().int().nullable(),
  cookMinutes: z.number().int().nullable(),
  freezable: z.boolean().nullable().describe('true si la recette se congèle bien'),
  ingredients: z.array(z.object({
    quantity: z.number().nullable(),
    unit: z.enum(UNITS).nullable(),
    name: z.string(),
    aisle: z.enum(AISLES).nullable().describe('rayon d\'épicerie'),
  })),
  steps: z.array(z.string()),
  tags: z.array(z.string()).describe('3 à 6 tags courts, en minuscules'),
  uncertainties: z.array(z.string()).describe('champs douteux à faire vérifier (illisible, ambigu)'),
})

const SYSTEM_PROMPT = `Tu extrais des recettes de cuisine vers un format structuré, sans jamais rien inventer.
Règles :
- Conserve la langue d'origine du texte (français ou anglais), sans traduire.
- Convertis les quantités vers les unités permises quand c'est possible (ex. 25 cl → 250 ml) ; sinon unit = null et la quantité reste le nombre d'unités.
- Si une quantité est illisible ou absente, mets null et signale-le dans uncertainties.
- aisle = le rayon d'épicerie le plus plausible pour l'ingrédient.
- Si le contenu ne contient pas de recette exploitable, found = false et laisse le reste vide.
- N'ajoute AUCUN texte hors du format demandé.`

export function isClaudeConfigured(): boolean {
  return Boolean(process.env.ANTHROPIC_API_KEY)
}

function extractionModel(): string {
  return process.env.NUXT_EXTRACTION_MODEL || DEFAULT_MODEL
}

/** Réduit le HTML en texte brut pour limiter les jetons envoyés. */
export function htmlToText(html: string): string {
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<(?:nav|footer|header|aside|form|svg)[\s\S]*?<\/(?:nav|footer|header|aside|form|svg)>/gi, ' ')
    .replace(/<br\s*\/?>|<\/(?:p|div|li|h[1-6]|tr)>/gi, '\n')
    .replace(/<[^>]+>/g, ' ')
    .replace(/[ \t]+/g, ' ')
    .replace(/\n\s*\n+/g, '\n')
    .trim()
    .slice(0, 60_000)
}

export interface ClaudeExtractionInput {
  html?: string
  images?: Array<{ mediaType: 'image/jpeg' | 'image/png' | 'image/webp', dataBase64: string }>
  sourceUrl?: string | null
}

export type ClaudeExtractionResult = { ok: true, draft: ImportDraft } | { ok: false, reason: 'notFound' | 'providerError' }

export async function extractRecipeWithClaude(input: ClaudeExtractionInput): Promise<ClaudeExtractionResult> {
  const client = new Anthropic()

  const content: Anthropic.ContentBlockParam[] = []
  for (const image of input.images ?? []) {
    content.push({
      type: 'image',
      source: { type: 'base64', media_type: image.mediaType, data: image.dataBase64 },
    })
  }
  content.push({
    type: 'text',
    text: input.html
      ? `Extrais la recette de cette page :\n\n${htmlToText(input.html)}`
      : 'Extrais la recette de ces photos (livre, fiche manuscrite ou capture d\'écran).',
  })

  let parsed: z.infer<typeof extractionSchema> | null
  try {
    const response = await client.messages.parse({
      model: extractionModel(),
      max_tokens: 8192,
      thinking: { type: 'adaptive' },
      system: SYSTEM_PROMPT,
      output_config: { format: zodOutputFormat(extractionSchema) },
      messages: [{ role: 'user', content }],
    })
    if (response.stop_reason === 'refusal') return { ok: false, reason: 'providerError' }
    parsed = response.parsed_output
  }
  catch {
    return { ok: false, reason: 'providerError' }
  }

  if (!parsed || !parsed.found || !parsed.title || parsed.ingredients.length === 0 || parsed.steps.length === 0) {
    return { ok: false, reason: 'notFound' }
  }

  const warnings = [...parsed.uncertainties]
  if (parsed.baseServings === null) warnings.push('Nombre de portions non détecté (défaut : 4).')

  const draft: ImportDraft = {
    title: parsed.title.slice(0, 200),
    description: parsed.description?.slice(0, 2000) ?? null,
    imageUrl: null,
    baseServings: Math.min(50, Math.max(1, parsed.baseServings ?? 4)),
    prepMinutes: parsed.prepMinutes,
    cookMinutes: parsed.cookMinutes,
    freezable: parsed.freezable ?? false,
    ingredients: parsed.ingredients.slice(0, 100).map((ingredient) => {
      if (ingredient.quantity === null) {
        warnings.push(`Quantité à vérifier : « ${ingredient.name} »`)
      }
      return {
        quantity: ingredient.quantity && ingredient.quantity > 0 ? Math.round(ingredient.quantity * 100) / 100 : 1,
        unit: ingredient.unit,
        name: ingredient.name.slice(0, 200),
        aisle: ingredient.aisle ?? guessAisle(ingredient.name),
      }
    }),
    steps: parsed.steps.slice(0, 50).map(step => step.slice(0, 2000)),
    tags: [...new Set(parsed.tags.map(t => t.trim().toLowerCase()).filter(t => t.length >= 2 && t.length <= 30))].slice(0, 8),
    sourceUrl: input.sourceUrl ?? null,
    warnings,
    provider: 'claude',
  }
  return { ok: true, draft }
}
