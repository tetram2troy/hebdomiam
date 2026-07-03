import { describe, expect, it } from 'vitest'
import { computePortions, mondayOf, pickSuggestions, type RotationCandidate } from '../../server/utils/rotation'

const NOW = new Date('2026-07-03T12:00:00Z')
const DAY = 24 * 60 * 60 * 1000

function daysAgo(n: number): Date {
  return new Date(NOW.getTime() - n * DAY)
}

let counter = 0
function candidate(overrides: Partial<RotationCandidate> = {}): RotationCandidate {
  counter += 1
  return {
    id: `recette-${String(counter).padStart(3, '0')}`,
    tags: [],
    lastCookedAt: null,
    createdAt: daysAgo(100),
    ...overrides,
  }
}

describe('computePortions', () => {
  it('rituel type : 7 jours × 2 repas × 1 personne / 4 plats', () => {
    expect(computePortions({ daysCount: 7, peopleCount: 1, dishCount: 4 }))
      .toEqual({ totalServings: 14, servingsPerDish: 4 })
  })

  it('arrondit toujours vers le haut (on ne manque jamais de portions)', () => {
    expect(computePortions({ daysCount: 5, peopleCount: 2, dishCount: 3 }))
      .toEqual({ totalServings: 20, servingsPerDish: 7 })
  })

  it('reste à 1 portion minimum par plat', () => {
    expect(computePortions({ daysCount: 1, peopleCount: 1, dishCount: 4 }).servingsPerDish).toBe(1)
  })
})

describe('pickSuggestions — exclusion par rotation', () => {
  it('exclut ce qui a été cuisiné dans les X dernières semaines', () => {
    const recent = candidate({ lastCookedAt: daysAgo(7) })
    const old = candidate({ lastCookedAt: daysAgo(30) })
    const never = candidate()

    const picked = pickSuggestions([recent, old, never], { count: 2, rotationWeeks: 3, now: NOW })
    expect(picked).toHaveLength(2)
    expect(picked).not.toContain(recent.id)
  })

  it('respecte la borne : cuisinée il y a exactement X semaines redevient admissible', () => {
    const boundary = candidate({ lastCookedAt: daysAgo(21) })
    const picked = pickSuggestions([boundary], { count: 1, rotationWeeks: 3, now: NOW })
    expect(picked).toEqual([boundary.id])
  })

  it('repli : collection trop petite → repêche les cuisinées récentes, les moins récentes d\'abord', () => {
    const yesterday = candidate({ lastCookedAt: daysAgo(1) })
    const lastWeek = candidate({ lastCookedAt: daysAgo(8) })
    const picked = pickSuggestions([yesterday, lastWeek], { count: 2, rotationWeeks: 3, now: NOW })
    expect(picked).toEqual([lastWeek.id, yesterday.id])
  })

  it('ignore les ids explicitement exclus (déjà au plan ou écartés)', () => {
    const a = candidate()
    const b = candidate()
    const picked = pickSuggestions([a, b], {
      count: 2,
      rotationWeeks: 3,
      now: NOW,
      excludeIds: new Set([a.id]),
    })
    expect(picked).toEqual([b.id])
  })
})

describe('pickSuggestions — priorités', () => {
  it('les jamais-cuisinées passent avant les cuisinées, même anciennes', () => {
    const cookedLongAgo = candidate({ lastCookedAt: daysAgo(300) })
    const never = candidate()
    const picked = pickSuggestions([cookedLongAgo, never], { count: 1, rotationWeeks: 3, now: NOW })
    expect(picked).toEqual([never.id])
  })

  it('entre cuisinées : la plus anciennement cuisinée d\'abord', () => {
    const a = candidate({ lastCookedAt: daysAgo(40) })
    const b = candidate({ lastCookedAt: daysAgo(90) })
    const picked = pickSuggestions([a, b], { count: 2, rotationWeeks: 3, now: NOW })
    expect(picked).toEqual([b.id, a.id])
  })

  it('est déterministe (départage stable par date de création puis id)', () => {
    const a = candidate({ createdAt: daysAgo(10) })
    const b = candidate({ createdAt: daysAgo(50) })
    for (let i = 0; i < 3; i++) {
      expect(pickSuggestions([a, b], { count: 2, rotationWeeks: 3, now: NOW })).toEqual([b.id, a.id])
    }
  })
})

describe('pickSuggestions — variété de tags', () => {
  it('évite deux plats du même tag dans la même semaine', () => {
    const pates1 = candidate({ tags: ['pâtes'], createdAt: daysAgo(90) })
    const pates2 = candidate({ tags: ['pâtes'], createdAt: daysAgo(80) })
    const soupe = candidate({ tags: ['soupe'], createdAt: daysAgo(70) })

    const picked = pickSuggestions([pates1, pates2, soupe], { count: 2, rotationWeeks: 3, now: NOW })
    expect(picked).toEqual([pates1.id, soupe.id])
  })

  it('à défaut de tag inédit, prend le chevauchement minimal', () => {
    const italien = candidate({ tags: ['pâtes', 'italien'], createdAt: daysAgo(90) })
    const doublon = candidate({ tags: ['pâtes', 'italien'], createdAt: daysAgo(80) })
    const cousin = candidate({ tags: ['pâtes'], createdAt: daysAgo(70) })

    const picked = pickSuggestions([italien, doublon, cousin], { count: 2, rotationWeeks: 3, now: NOW })
    expect(picked).toEqual([italien.id, cousin.id])
  })

  it('les recettes sans tag ne bloquent jamais la diversité', () => {
    const sansTag1 = candidate()
    const sansTag2 = candidate()
    const picked = pickSuggestions([sansTag1, sansTag2], { count: 2, rotationWeeks: 3, now: NOW })
    expect(picked).toHaveLength(2)
  })
})

describe('mondayOf', () => {
  it('retourne le lundi de la semaine courante', () => {
    expect(mondayOf(new Date(2026, 6, 3))).toBe('2026-06-29') // vendredi 3 juillet
    expect(mondayOf(new Date(2026, 6, 5))).toBe('2026-06-29') // dimanche 5 juillet
    expect(mondayOf(new Date(2026, 5, 29))).toBe('2026-06-29') // lundi lui-même
  })
})

describe('pickSuggestions — bords', () => {
  it('demande plus grand que la collection → retourne tout, sans doublon', () => {
    const a = candidate()
    const b = candidate()
    const picked = pickSuggestions([a, b], { count: 4, rotationWeeks: 3, now: NOW })
    expect(picked.sort()).toEqual([a.id, b.id].sort())
  })

  it('count nul ou collection vide → liste vide', () => {
    expect(pickSuggestions([], { count: 4, rotationWeeks: 3, now: NOW })).toEqual([])
    expect(pickSuggestions([candidate()], { count: 0, rotationWeeks: 3, now: NOW })).toEqual([])
  })
})
