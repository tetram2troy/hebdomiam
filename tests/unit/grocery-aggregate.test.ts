import { describe, expect, it } from 'vitest'
import { aggregateIngredients, normalizeIngredientName, type AggregationInput } from '../../server/utils/grocery-aggregate'
import { dimensionOf, displayQuantity, toBase } from '../../server/utils/units'

let counter = 0
function ing(overrides: Partial<AggregationInput> = {}): AggregationInput {
  counter += 1
  return {
    ingredientId: `ing-${counter}`,
    name: 'riz basmati',
    quantity: 200,
    unit: 'g',
    aisle: 'dry',
    recipeTitle: `Recette ${counter}`,
    baseServings: 4,
    targetServings: 4,
    ...overrides,
  }
}

describe('unités — conversion et dimensions', () => {
  it('classe chaque unité dans sa dimension', () => {
    expect(dimensionOf('g')).toBe('mass')
    expect(dimensionOf('lb')).toBe('mass')
    expect(dimensionOf('cup')).toBe('volume')
    expect(dimensionOf('unit')).toBe('count')
    expect(dimensionOf('pinch')).toBe('pinch')
    expect(dimensionOf(null)).toBe('none')
  })

  it('convertit vers l\'unité de base (tasses canadiennes : 250 ml)', () => {
    expect(toBase(2, 'kg')).toBe(2000)
    expect(toBase(1, 'lb')).toBeCloseTo(453.59, 1)
    expect(toBase(2, 'cup')).toBe(500)
    expect(toBase(3, 'tbsp')).toBe(45)
    expect(toBase(5, 'unit')).toBe(5)
  })

  it('affichage métrique : bascule g→kg et ml→L à 1000', () => {
    expect(displayQuantity(350, 'g', 'metric')).toEqual({ quantity: 350, unit: 'g' })
    expect(displayQuantity(1500, 'g', 'metric')).toEqual({ quantity: 1.5, unit: 'kg' })
    expect(displayQuantity(800, 'ml', 'metric')).toEqual({ quantity: 800, unit: 'ml' })
    expect(displayQuantity(2250, 'ml', 'metric')).toEqual({ quantity: 2.25, unit: 'l' })
  })

  it('affichage impérial : oz/lb pour la masse, tsp/tbsp/tasse pour le volume', () => {
    expect(displayQuantity(453.592, 'g', 'imperial')).toEqual({ quantity: 1, unit: 'lb' })
    expect(displayQuantity(100, 'g', 'imperial')).toEqual({ quantity: 3.53, unit: 'oz' })
    expect(displayQuantity(500, 'ml', 'imperial')).toEqual({ quantity: 2, unit: 'cup' })
    expect(displayQuantity(30, 'ml', 'imperial')).toEqual({ quantity: 2, unit: 'tbsp' })
    expect(displayQuantity(5, 'ml', 'imperial')).toEqual({ quantity: 1, unit: 'tsp' })
  })

  it('les unités de comptage restent inchangées dans les deux systèmes', () => {
    expect(displayQuantity(5, 'unit', 'imperial')).toEqual({ quantity: 5, unit: 'unit' })
    expect(displayQuantity(2, 'pinch', 'metric')).toEqual({ quantity: 2, unit: 'pinch' })
  })
})

describe('agrégation — fusion des ingrédients', () => {
  it('additionne les quantités de même nom et même dimension (2 recettes de riz)', () => {
    const items = aggregateIngredients([
      ing({ quantity: 200, recipeTitle: 'Poulet au beurre' }),
      ing({ quantity: 150, recipeTitle: 'Tofu Tao' }),
    ])
    expect(items).toHaveLength(1)
    expect(items[0]).toMatchObject({ label: 'riz basmati', quantity: 350, unit: 'g', aisle: 'dry' })
    expect(items[0]!.sources.map(s => s.recipeTitle)).toEqual(['Poulet au beurre', 'Tofu Tao'])
  })

  it('convertit avant d\'additionner (200 g + 0,5 kg = 700 g)', () => {
    const items = aggregateIngredients([
      ing({ quantity: 200, unit: 'g' }),
      ing({ quantity: 0.5, unit: 'kg' }),
    ])
    expect(items[0]!.quantity).toBe(700)
    expect(items[0]!.unit).toBe('g')
  })

  it('additionne les volumes en tasses et millilitres (1 tasse + 250 ml = 500 ml)', () => {
    const items = aggregateIngredients([
      ing({ name: 'bouillon de légumes', quantity: 1, unit: 'cup', aisle: 'canned' }),
      ing({ name: 'bouillon de légumes', quantity: 250, unit: 'ml', aisle: 'canned' }),
    ])
    expect(items).toHaveLength(1)
    expect(items[0]).toMatchObject({ quantity: 500, unit: 'ml' })
  })

  it('met à l\'échelle selon les portions du plan avant d\'agréger', () => {
    const items = aggregateIngredients([
      ing({ quantity: 200, baseServings: 4, targetServings: 6 }), // ×1,5 = 300
      ing({ quantity: 100, baseServings: 2, targetServings: 5 }), // ×2,5 = 250
    ])
    expect(items[0]!.quantity).toBe(550)
    expect(items[0]!.sources.map(s => s.quantity)).toEqual([300, 250])
  })

  it('garde des lignes séparées quand les unités sont incompatibles (« 2 tomates » + « 200 g »)', () => {
    const items = aggregateIngredients([
      ing({ name: 'tomates', quantity: 2, unit: 'unit', aisle: 'produce' }),
      ing({ name: 'tomates', quantity: 200, unit: 'g', aisle: 'produce' }),
    ])
    expect(items).toHaveLength(2)
    expect(items.map(i => i.unit).sort()).toEqual(['g', 'unit'])
  })

  it('fusionne malgré la casse et les espaces (normalisation du nom)', () => {
    expect(normalizeIngredientName('  Lentilles  Vertes ')).toBe('lentilles vertes')
    const items = aggregateIngredients([
      ing({ name: 'Lentilles vertes' }),
      ing({ name: 'lentilles  VERTES' }),
    ])
    expect(items).toHaveLength(1)
    expect(items[0]!.label).toBe('Lentilles vertes') // libellé de la première occurrence
  })

  it('ne fusionne pas des noms différents', () => {
    const items = aggregateIngredients([
      ing({ name: 'oignons jaunes' }),
      ing({ name: 'oignons rouges' }),
    ])
    expect(items).toHaveLength(2)
  })

  it('un rayon précis l\'emporte sur « other »', () => {
    const items = aggregateIngredients([
      ing({ aisle: 'other' }),
      ing({ aisle: 'dry' }),
    ])
    expect(items[0]!.aisle).toBe('dry')
  })

  it('additionne les unités de comptage et les pincées entre elles', () => {
    const items = aggregateIngredients([
      ing({ name: 'oignons', quantity: 2, unit: 'unit' }),
      ing({ name: 'oignons', quantity: 3, unit: 'unit' }),
      ing({ name: 'sel', quantity: 1, unit: 'pinch' }),
      ing({ name: 'sel', quantity: 2, unit: 'pinch' }),
    ])
    const oignons = items.find(i => i.label === 'oignons')!
    const sel = items.find(i => i.label === 'sel')!
    expect(oignons).toMatchObject({ quantity: 5, unit: 'unit' })
    expect(sel).toMatchObject({ quantity: 3, unit: 'pinch' })
  })

  it('évite le bruit des flottants (3 × 0,1 kg = 300 g exactement)', () => {
    const items = aggregateIngredients([
      ing({ quantity: 0.1, unit: 'kg' }),
      ing({ quantity: 0.1, unit: 'kg' }),
      ing({ quantity: 0.1, unit: 'kg' }),
    ])
    expect(items[0]!.quantity).toBe(300)
  })
})
