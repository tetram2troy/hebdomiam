import type { Aisle, Unit } from '#shared/schemas/recipe'
import type { GroceryItemSource } from '../database/schema'
import { baseUnitOf, dimensionOf, toBase } from './units'

// Agrégation de la liste d'épicerie (PRD F6) — fonction pure.
// Deux ingrédients fusionnent si même nom normalisé ET même dimension d'unité ;
// sinon les lignes restent séparées (« 2 tomates » + « 200 g de tomates »).

export interface AggregationInput {
  ingredientId: string
  name: string
  quantity: number
  unit: Unit | null
  aisle: Aisle
  recipeTitle: string
  baseServings: number
  targetServings: number
}

export interface AggregatedItem {
  label: string
  /** Quantité totale en unité de base (g / ml / unités / pincées). */
  quantity: number
  /** Unité de base de la dimension (g, ml, unit, pinch) ou null. */
  unit: Unit | null
  aisle: Aisle
  sources: GroceryItemSource[]
}

export function normalizeIngredientName(name: string): string {
  return name.trim().toLowerCase().replace(/\s+/g, ' ')
}

export function aggregateIngredients(rows: AggregationInput[]): AggregatedItem[] {
  const groups = new Map<string, AggregatedItem>()

  for (const row of rows) {
    const scaled = row.quantity * row.targetServings / row.baseServings
    const dimension = dimensionOf(row.unit)
    const key = `${normalizeIngredientName(row.name)}|${dimension}`

    let group = groups.get(key)
    if (!group) {
      group = {
        label: row.name.trim(),
        quantity: 0,
        unit: baseUnitOf(dimension),
        aisle: row.aisle,
        sources: [],
      }
      groups.set(key, group)
    }

    group.quantity += toBase(scaled, row.unit)
    // Premier rayon significatif : « other » cède la place à un rayon précis.
    if (group.aisle === 'other' && row.aisle !== 'other') {
      group.aisle = row.aisle
    }
    group.sources.push({
      ingredientId: row.ingredientId,
      recipeTitle: row.recipeTitle,
      quantity: Math.round(scaled * 100) / 100,
      unit: row.unit,
    })
  }

  return [...groups.values()].map(group => ({
    ...group,
    quantity: Math.round(group.quantity * 100) / 100,
  }))
}
