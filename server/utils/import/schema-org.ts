import type { ImportDraft } from '#shared/types/import'
import { parseIngredientLine } from './ingredient-line'

// Extraction schema.org/Recipe depuis le HTML d'une page (JSON-LD) — pur, testé.
// Gratuit et instantané : toujours essayé avant le repli IA (PRD F3, D1).

/** « PT1H30M » → 90 minutes. Retourne null si illisible. */
export function parseIsoDuration(value: unknown): number | null {
  if (typeof value !== 'string') return null
  const match = value.match(/^-?P(?:(\d+(?:\.\d+)?)D)?T?(?:(\d+(?:\.\d+)?)H)?(?:(\d+(?:\.\d+)?)M)?(?:(\d+(?:\.\d+)?)S)?$/i)
  if (!match || (!match[1] && !match[2] && !match[3] && !match[4])) return null
  const minutes = (Number(match[1] ?? 0) * 24 * 60)
    + (Number(match[2] ?? 0) * 60)
    + Number(match[3] ?? 0)
    + (Number(match[4] ?? 0) / 60)
  const rounded = Math.round(minutes)
  return rounded > 0 ? rounded : null
}

const NAMED_ENTITIES: Record<string, string> = {
  amp: '&', lt: '<', gt: '>', quot: '"', apos: '\'', nbsp: ' ',
  eacute: 'é', egrave: 'è', ecirc: 'ê', euml: 'ë',
  agrave: 'à', acirc: 'â', icirc: 'î', iuml: 'ï',
  ocirc: 'ô', ucirc: 'û', ugrave: 'ù', uuml: 'ü',
  ccedil: 'ç', oelig: 'œ', rsquo: '’', lsquo: '‘',
  laquo: '«', raquo: '»', deg: '°', frac12: '½', frac14: '¼', frac34: '¾',
}

function decodeEntities(text: string): string {
  return text
    .replace(/&#(\d+);/g, (_, code) => String.fromCodePoint(Number(code)))
    .replace(/&#x([0-9a-f]+);/gi, (_, code) => String.fromCodePoint(Number.parseInt(code, 16)))
    .replace(/&([a-z]+[0-9]*);/gi, (full, name) => NAMED_ENTITIES[String(name).toLowerCase()] ?? full)
}

function asText(value: unknown): string | null {
  if (typeof value === 'string') {
    const cleaned = decodeEntities(value.replace(/<[^>]+>/g, ' ')).replace(/\s+/g, ' ').trim()
    return cleaned || null
  }
  return null
}

type JsonObject = Record<string, unknown>

function isRecipeNode(node: unknown): node is JsonObject {
  if (typeof node !== 'object' || node === null) return false
  const type = (node as JsonObject)['@type']
  if (typeof type === 'string') return type.toLowerCase() === 'recipe'
  if (Array.isArray(type)) return type.some(t => typeof t === 'string' && t.toLowerCase() === 'recipe')
  return false
}

/** Cherche un nœud Recipe dans un document JSON-LD (gère @graph et les tableaux). */
function findRecipeNode(data: unknown): JsonObject | null {
  if (isRecipeNode(data)) return data
  if (Array.isArray(data)) {
    for (const item of data) {
      const found = findRecipeNode(item)
      if (found) return found
    }
    return null
  }
  if (typeof data === 'object' && data !== null) {
    const graph = (data as JsonObject)['@graph']
    if (graph) return findRecipeNode(graph)
  }
  return null
}

function extractServings(recipeYield: unknown): number | null {
  const candidates = Array.isArray(recipeYield) ? recipeYield : [recipeYield]
  for (const candidate of candidates) {
    if (typeof candidate === 'number' && Number.isFinite(candidate)) {
      return Math.min(50, Math.max(1, Math.round(candidate)))
    }
    if (typeof candidate === 'string') {
      const match = candidate.match(/\d+/)
      if (match) return Math.min(50, Math.max(1, Number.parseInt(match[0])))
    }
  }
  return null
}

function extractSteps(instructions: unknown): string[] {
  const steps: string[] = []
  function walk(node: unknown) {
    if (typeof node === 'string') {
      const text = asText(node)
      if (text) steps.push(text)
      return
    }
    if (Array.isArray(node)) {
      for (const item of node) walk(item)
      return
    }
    if (typeof node === 'object' && node !== null) {
      const obj = node as JsonObject
      // HowToSection → itemListElement ; HowToStep → text
      if (obj.itemListElement) {
        walk(obj.itemListElement)
        return
      }
      const text = asText(obj.text) ?? asText(obj.name)
      if (text) steps.push(text)
    }
  }
  walk(instructions)
  return steps.map(s => s.slice(0, 2000)).slice(0, 50)
}

function extractImage(image: unknown): string | null {
  if (typeof image === 'string') return image.startsWith('http') ? image.slice(0, 1000) : null
  if (Array.isArray(image)) {
    for (const item of image) {
      const found = extractImage(item)
      if (found) return found
    }
    return null
  }
  if (typeof image === 'object' && image !== null) {
    return extractImage((image as JsonObject).url)
  }
  return null
}

function extractTags(node: JsonObject): string[] {
  const raw: unknown[] = []
  for (const key of ['keywords', 'recipeCategory', 'recipeCuisine']) {
    const value = node[key]
    if (typeof value === 'string') raw.push(...value.split(','))
    else if (Array.isArray(value)) raw.push(...value)
  }
  const tags = raw
    .map(t => (typeof t === 'string' ? decodeEntities(t).trim().toLowerCase() : ''))
    .filter(t => t.length >= 2 && t.length <= 30)
  return [...new Set(tags)].slice(0, 8)
}

/** Extrait un brouillon de recette du HTML, ou null si aucun JSON-LD Recipe. */
export function extractRecipeFromHtml(html: string, sourceUrl: string): ImportDraft | null {
  const scripts = html.matchAll(/<script[^>]*type\s*=\s*["']application\/ld\+json["'][^>]*>([\s\S]*?)<\/script>/gi)

  let recipe: JsonObject | null = null
  for (const match of scripts) {
    try {
      recipe = findRecipeNode(JSON.parse(match[1]!.trim()))
      if (recipe) break
    }
    catch {
      // JSON-LD invalide sur la page : on tente le bloc suivant.
    }
  }
  if (!recipe) return null

  const title = asText(recipe.name)
  if (!title) return null

  const warnings: string[] = []

  const rawIngredients = Array.isArray(recipe.recipeIngredient)
    ? recipe.recipeIngredient
    : (Array.isArray(recipe.ingredients) ? recipe.ingredients : [])
  const ingredients = rawIngredients
    .map(line => asText(line))
    .filter((line): line is string => Boolean(line))
    .slice(0, 100)
    .map((line) => {
      const parsed = parseIngredientLine(line)
      if (!parsed.parsed) warnings.push(`Quantité à vérifier : « ${line} »`)
      return { quantity: parsed.quantity, unit: parsed.unit, name: parsed.name.slice(0, 200), aisle: parsed.aisle }
    })

  const steps = extractSteps(recipe.recipeInstructions)
  if (ingredients.length === 0 || steps.length === 0) return null

  const baseServings = extractServings(recipe.recipeYield)
  if (baseServings === null) warnings.push('Nombre de portions non détecté (défaut : 4).')

  const prepMinutes = parseIsoDuration(recipe.prepTime)
  let cookMinutes = parseIsoDuration(recipe.cookTime)
  if (cookMinutes === null) {
    const total = parseIsoDuration(recipe.totalTime)
    if (total !== null) cookMinutes = Math.max(0, total - (prepMinutes ?? 0)) || null
  }

  return {
    title: title.slice(0, 200),
    description: asText(recipe.description)?.slice(0, 2000) ?? null,
    imageUrl: extractImage(recipe.image),
    baseServings: baseServings ?? 4,
    prepMinutes,
    cookMinutes,
    freezable: false,
    ingredients,
    steps,
    tags: extractTags(recipe),
    sourceUrl,
    warnings,
    provider: 'schema-org',
  }
}
