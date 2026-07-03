// Rotation intelligente (PRD F5) — fonctions pures, sans DB, testées unitairement.
// Règles : exclure ce qui a été cuisiné dans les X dernières semaines, prioriser
// jamais-cuisinées puis les plus anciennement cuisinées, maximiser la variété de
// tags au sein d'une même semaine. Repli si la collection est trop petite.

export interface RotationCandidate {
  id: string
  tags: string[]
  lastCookedAt: Date | null
  createdAt: Date
}

export interface RotationOptions {
  count: number
  rotationWeeks: number
  excludeIds?: ReadonlySet<string>
  now?: Date
}

export interface PortionParams {
  daysCount: number
  peopleCount: number
  dishCount: number
  /** Dîners + soupers en V1 (décision D7). */
  mealsPerDay?: number
}

export function computePortions({ daysCount, peopleCount, dishCount, mealsPerDay = 2 }: PortionParams) {
  const totalServings = daysCount * mealsPerDay * peopleCount
  const servingsPerDish = Math.max(1, Math.ceil(totalServings / dishCount))
  return { totalServings, servingsPerDish }
}

/** Lundi de la semaine de `date`, au fuseau local, au format YYYY-MM-DD. */
export function mondayOf(date: Date): string {
  const local = new Date(date.getFullYear(), date.getMonth(), date.getDate())
  const shift = (local.getDay() + 6) % 7 // lundi = 0
  local.setDate(local.getDate() - shift)
  const month = String(local.getMonth() + 1).padStart(2, '0')
  const day = String(local.getDate()).padStart(2, '0')
  return `${local.getFullYear()}-${month}-${day}`
}

/** Jamais cuisinées d'abord (les plus anciennes de la collection en premier),
    puis par dernière cuisson la plus lointaine. Départage stable par id. */
function byPriority(a: RotationCandidate, b: RotationCandidate): number {
  const aNever = a.lastCookedAt === null
  const bNever = b.lastCookedAt === null
  if (aNever !== bNever) return aNever ? -1 : 1
  if (!aNever && a.lastCookedAt!.getTime() !== b.lastCookedAt!.getTime()) {
    return a.lastCookedAt!.getTime() - b.lastCookedAt!.getTime()
  }
  if (a.createdAt.getTime() !== b.createdAt.getTime()) {
    return a.createdAt.getTime() - b.createdAt.getTime()
  }
  return a.id.localeCompare(b.id)
}

function tagOverlap(tags: string[], picked: ReadonlySet<string>): number {
  return tags.reduce((n, tag) => n + (picked.has(tag) ? 1 : 0), 0)
}

/** Retourne jusqu'à `count` ids de recettes, dans l'ordre de suggestion. */
export function pickSuggestions(candidates: RotationCandidate[], options: RotationOptions): string[] {
  const { count, rotationWeeks, excludeIds, now = new Date() } = options
  if (count <= 0) return []

  const cutoff = now.getTime() - rotationWeeks * 7 * 24 * 60 * 60 * 1000
  const eligible = candidates.filter(c => !excludeIds?.has(c.id))
  const fresh = eligible.filter(c => c.lastCookedAt === null || c.lastCookedAt.getTime() <= cutoff)

  // Repli : collection trop petite pour respecter la rotation → on repêche les
  // cuisinées récemment (le tri les met naturellement en dernier).
  const pool = [...(fresh.length >= count ? fresh : eligible)].sort(byPriority)

  const picked: RotationCandidate[] = []
  const pickedTags = new Set<string>()
  while (picked.length < count && pool.length > 0) {
    let bestIndex = 0
    let bestOverlap = Number.POSITIVE_INFINITY
    for (const [index, candidate] of pool.entries()) {
      const overlap = tagOverlap(candidate.tags, pickedTags)
      if (overlap === 0) {
        bestIndex = index
        break
      }
      if (overlap < bestOverlap) {
        bestOverlap = overlap
        bestIndex = index
      }
    }
    const [choice] = pool.splice(bestIndex, 1)
    picked.push(choice!)
    for (const tag of choice!.tags) pickedTags.add(tag)
  }

  return picked.map(c => c.id)
}
