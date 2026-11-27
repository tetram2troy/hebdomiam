import { describe, expect, it } from 'vitest'
import { filterTags, normalizeForSearch } from '../../app/utils/tag-filter'

const TAGS = [
  { name: 'végé', recipeCount: 5 },
  { name: 'mijoté', recipeCount: 4 },
  { name: 'se congèle', recipeCount: 3 },
  { name: 'épicé', recipeCount: 1 },
  { name: 'bbq', recipeCount: 1 },
]

describe('normalizeForSearch', () => {
  it('ignore la casse, les accents et les espaces autour', () => {
    expect(normalizeForSearch('  Végé ')).toBe('vege')
    expect(normalizeForSearch('SE CONGÈLE')).toBe('se congele')
    expect(normalizeForSearch('crème brûlée')).toBe('creme brulee')
  })
})

describe('filterTags', () => {
  it('recherche vide ou blanche : tous les tags, même tableau', () => {
    expect(filterTags(TAGS, '')).toBe(TAGS)
    expect(filterTags(TAGS, '   ')).toBe(TAGS)
  })

  it('trouve sans accent un tag accentué, et inversement', () => {
    expect(filterTags(TAGS, 'vege').map(t => t.name)).toEqual(['végé'])
    expect(filterTags(TAGS, 'ÉPICE').map(t => t.name)).toEqual(['épicé'])
    expect(filterTags(TAGS, 'bbq').map(t => t.name)).toEqual(['bbq'])
  })

  it('cherche au milieu du nom et conserve l\'ordre d\'entrée', () => {
    expect(filterTags(TAGS, 'é').map(t => t.name)).toEqual(['végé', 'mijoté', 'se congèle', 'épicé'])
    expect(filterTags(TAGS, 'gel').map(t => t.name)).toEqual(['se congèle'])
  })

  it('aucune correspondance : liste vide', () => {
    expect(filterTags(TAGS, 'poisson')).toEqual([])
  })
})
