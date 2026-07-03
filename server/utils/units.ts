import type { Unit } from '#shared/schemas/recipe'

// Normalisation d'unités (PRD F6, décision D6) — fonctions pures.
// Les quantités agrégées sont stockées en unité de BASE (g / ml) ; l'affichage
// convertit selon la préférence utilisateur (métrique ou impérial/tasses).

export type Dimension = 'mass' | 'volume' | 'count' | 'pinch' | 'none'

/** Facteurs vers l'unité de base (g pour la masse, ml pour le volume).
    Tasses canadiennes : 1 tasse = 250 ml. */
const TO_BASE: Record<string, { dimension: Dimension, factor: number }> = {
  g: { dimension: 'mass', factor: 1 },
  kg: { dimension: 'mass', factor: 1000 },
  oz: { dimension: 'mass', factor: 28.35 },
  lb: { dimension: 'mass', factor: 453.592 },
  ml: { dimension: 'volume', factor: 1 },
  l: { dimension: 'volume', factor: 1000 },
  tsp: { dimension: 'volume', factor: 5 },
  tbsp: { dimension: 'volume', factor: 15 },
  cup: { dimension: 'volume', factor: 250 },
}

export function dimensionOf(unit: Unit | null): Dimension {
  if (unit === null) return 'none'
  if (unit === 'unit') return 'count'
  if (unit === 'pinch') return 'pinch'
  return TO_BASE[unit]!.dimension
}

/** Convertit vers l'unité de base de sa dimension (g, ml) ; les dimensions
    sans conversion (count, pinch, none) passent telles quelles. */
export function toBase(quantity: number, unit: Unit | null): number {
  const entry = unit === null ? undefined : TO_BASE[unit]
  return entry ? quantity * entry.factor : quantity
}

export interface DisplayQuantity {
  quantity: number
  unit: Unit | null
}

function round(value: number): number {
  return Math.round(value * 100) / 100
}

/** Unité de base d'une dimension (celle stockée en base de données). */
export function baseUnitOf(dimension: Dimension): Unit | null {
  switch (dimension) {
    case 'mass': return 'g'
    case 'volume': return 'ml'
    case 'count': return 'unit'
    case 'pinch': return 'pinch'
    case 'none': return null
  }
}

/** Convertit une quantité en unité de base vers l'unité d'affichage la plus
    lisible du système demandé. */
export function displayQuantity(baseQuantity: number, unit: Unit | null, system: 'metric' | 'imperial'): DisplayQuantity {
  const dimension = dimensionOf(unit)

  if (dimension === 'mass') {
    if (system === 'imperial') {
      return baseQuantity >= TO_BASE.lb!.factor
        ? { quantity: round(baseQuantity / TO_BASE.lb!.factor), unit: 'lb' }
        : { quantity: round(baseQuantity / TO_BASE.oz!.factor), unit: 'oz' }
    }
    return baseQuantity >= 1000
      ? { quantity: round(baseQuantity / 1000), unit: 'kg' }
      : { quantity: round(baseQuantity), unit: 'g' }
  }

  if (dimension === 'volume') {
    if (system === 'imperial') {
      if (baseQuantity >= 60) return { quantity: round(baseQuantity / TO_BASE.cup!.factor), unit: 'cup' }
      if (baseQuantity >= 15) return { quantity: round(baseQuantity / TO_BASE.tbsp!.factor), unit: 'tbsp' }
      return { quantity: round(baseQuantity / TO_BASE.tsp!.factor), unit: 'tsp' }
    }
    return baseQuantity >= 1000
      ? { quantity: round(baseQuantity / 1000), unit: 'l' }
      : { quantity: round(baseQuantity), unit: 'ml' }
  }

  return { quantity: round(baseQuantity), unit }
}
