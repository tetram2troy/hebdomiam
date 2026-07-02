import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { recipeInputSchema } from '../../shared/schemas/recipe'
import {
  createRecipe,
  getRecipe,
  listRecipes,
  listTags,
  softDeleteRecipe,
  updateRecipe,
} from '../../server/utils/recipe-repo'
import { connectTestDb, createTestUser, truncateAll } from './test-db'

const db = connectTestDb()

function chiliInput(overrides: Record<string, unknown> = {}) {
  return recipeInputSchema.parse({
    title: 'Chili de lentilles à la mijoteuse',
    description: 'Un classique réconfortant.',
    baseServings: 4,
    prepMinutes: 15,
    cookMinutes: 30,
    freezable: true,
    ingredients: [
      { quantity: 500, unit: 'g', name: 'lentilles vertes', aisle: 'dry' },
      { quantity: 800, unit: 'ml', name: 'tomates en dés', aisle: 'canned' },
      { quantity: 2, unit: 'unit', name: 'oignons jaunes', aisle: 'produce' },
    ],
    steps: ['Hacher les oignons.', 'Tout verser dans la mijoteuse.', 'Cuire 3 h à intensité élevée.'],
    tags: ['végé', 'mijoteuse'],
    ...overrides,
  })
}

beforeEach(async () => {
  await truncateAll(db)
})

afterAll(async () => {
  await db.$client.end()
})

describe('createRecipe / getRecipe', () => {
  it('crée une recette complète et la relit fidèlement', async () => {
    const user = await createTestUser(db)
    const created = await createRecipe(db, user.id, chiliInput())

    expect(created.title).toBe('Chili de lentilles à la mijoteuse')
    expect(created.freezable).toBe(true)
    expect(created.tags).toEqual(['mijoteuse', 'végé'])
    expect(created.steps.map(s => s.position)).toEqual([1, 2, 3])
    expect(created.ingredients).toHaveLength(3)
    // decimal relu en number, pas en chaîne
    expect(created.ingredients[0]!.quantity).toBe(500)
    expect(created.ingredients[0]!.aisle).toBe('dry')

    const read = await getRecipe(db, user.id, created.id)
    expect(read).not.toBeNull()
    expect(read!.steps.map(s => s.text)).toEqual([
      'Hacher les oignons.',
      'Tout verser dans la mijoteuse.',
      'Cuire 3 h à intensité élevée.',
    ])
  })

  it('conserve les décimales des quantités (decimal, jamais float)', async () => {
    const user = await createTestUser(db)
    const created = await createRecipe(db, user.id, chiliInput({
      ingredients: [{ quantity: 1.25, unit: 'cup', name: 'riz', aisle: 'dry' }],
    }))
    expect(created.ingredients[0]!.quantity).toBe(1.25)
  })

  it('réutilise les tags existants sans doublon (unicité userId+nom)', async () => {
    const user = await createTestUser(db)
    await createRecipe(db, user.id, chiliInput())
    await createRecipe(db, user.id, chiliInput({ title: 'Dahl corail', tags: ['végé', 'indien'] }))

    const tags = await listTags(db, user.id)
    expect(tags).toEqual([
      { name: 'végé', recipeCount: 2 },
      { name: 'indien', recipeCount: 1 },
      { name: 'mijoteuse', recipeCount: 1 },
    ])
  })
})

describe('listRecipes — recherche et filtres', () => {
  it('filtre par titre ou par ingrédient (q)', async () => {
    const user = await createTestUser(db)
    await createRecipe(db, user.id, chiliInput())
    await createRecipe(db, user.id, chiliInput({
      title: 'Poulet au beurre',
      tags: [],
      ingredients: [{ quantity: 900, unit: 'g', name: 'hauts de cuisse de poulet', aisle: 'butcher' }],
    }))

    const byTitle = await listRecipes(db, user.id, { q: 'chili' })
    expect(byTitle.map(r => r.title)).toEqual(['Chili de lentilles à la mijoteuse'])

    const byIngredient = await listRecipes(db, user.id, { q: 'lentilles' })
    expect(byIngredient.map(r => r.title)).toEqual(['Chili de lentilles à la mijoteuse'])

    const none = await listRecipes(db, user.id, { q: 'bourguignon' })
    expect(none).toEqual([])
  })

  it('filtre par tag et par temps total maximal', async () => {
    const user = await createTestUser(db)
    await createRecipe(db, user.id, chiliInput()) // 45 min, végé
    await createRecipe(db, user.id, chiliInput({
      title: 'Lasagne courge', prepMinutes: 30, cookMinutes: 60, tags: ['four'],
    }))

    const byTag = await listRecipes(db, user.id, { tag: 'végé' })
    expect(byTag.map(r => r.title)).toEqual(['Chili de lentilles à la mijoteuse'])

    const quick = await listRecipes(db, user.id, { maxMinutes: 45 })
    expect(quick.map(r => r.title)).toEqual(['Chili de lentilles à la mijoteuse'])
  })
})

describe('updateRecipe', () => {
  it('remplace les enfants (étapes, ingrédients, tags) sans orphelins', async () => {
    const user = await createTestUser(db)
    const created = await createRecipe(db, user.id, chiliInput())

    const updated = await updateRecipe(db, user.id, created.id, chiliInput({
      title: 'Chili revisité',
      steps: ['Nouvelle étape unique.'],
      ingredients: [{ quantity: 300, unit: 'g', name: 'lentilles corail', aisle: 'dry' }],
      tags: ['végé'],
    }))

    expect(updated).not.toBeNull()
    expect(updated!.title).toBe('Chili revisité')
    expect(updated!.steps).toHaveLength(1)
    expect(updated!.ingredients).toHaveLength(1)
    expect(updated!.tags).toEqual(['végé'])
    // le tag « mijoteuse » n'est plus rattaché
    const tags = await listTags(db, user.id)
    expect(tags.find(t => t.name === 'mijoteuse')).toBeUndefined()
  })
})

describe('softDeleteRecipe', () => {
  it('masque la recette partout sans détruire la ligne', async () => {
    const user = await createTestUser(db)
    const created = await createRecipe(db, user.id, chiliInput())

    expect(await softDeleteRecipe(db, user.id, created.id)).toBe(true)
    expect(await getRecipe(db, user.id, created.id)).toBeNull()
    expect(await listRecipes(db, user.id)).toEqual([])
    expect(await listTags(db, user.id)).toEqual([])
    // une seconde suppression ne trouve plus rien
    expect(await softDeleteRecipe(db, user.id, created.id)).toBe(false)
  })
})

describe('isolation multi-utilisateur (IDOR)', () => {
  it('un autre utilisateur ne peut ni lire, ni modifier, ni supprimer', async () => {
    const alice = await createTestUser(db, 'Alice')
    const bob = await createTestUser(db, 'Bob')
    const recipe = await createRecipe(db, alice.id, chiliInput())

    expect(await getRecipe(db, bob.id, recipe.id)).toBeNull()
    expect(await updateRecipe(db, bob.id, recipe.id, chiliInput({ title: 'Piraté' }))).toBeNull()
    expect(await softDeleteRecipe(db, bob.id, recipe.id)).toBe(false)

    // rien n'a fuité ni changé pour Alice
    const intact = await getRecipe(db, alice.id, recipe.id)
    expect(intact!.title).toBe('Chili de lentilles à la mijoteuse')
    expect(await listRecipes(db, bob.id)).toEqual([])
    expect(await listTags(db, bob.id)).toEqual([])
  })

  it('le filtre par tag ne fuit pas les tags homonymes d\'un autre compte', async () => {
    const alice = await createTestUser(db, 'Alice')
    const bob = await createTestUser(db, 'Bob')
    await createRecipe(db, alice.id, chiliInput())
    await createRecipe(db, bob.id, chiliInput({ title: 'Chili de Bob' }))

    const bobVege = await listRecipes(db, bob.id, { tag: 'végé' })
    expect(bobVege.map(r => r.title)).toEqual(['Chili de Bob'])
  })
})

describe('validation Zod (recipeInputSchema)', () => {
  it('rejette une recette sans ingrédient ou sans étape', () => {
    expect(() => chiliInput({ ingredients: [] })).toThrow()
    expect(() => chiliInput({ steps: [] })).toThrow()
  })

  it('rejette une quantité négative et une unité inconnue', () => {
    expect(() => chiliInput({
      ingredients: [{ quantity: -1, unit: 'g', name: 'x', aisle: 'dry' }],
    })).toThrow()
    expect(() => chiliInput({
      ingredients: [{ quantity: 1, unit: 'poignée', name: 'x', aisle: 'dry' }],
    })).toThrow()
  })

  it('déduplique les tags et applique les défauts', () => {
    const input = chiliInput({ tags: ['végé', 'végé'] })
    expect(input.tags).toEqual(['végé'])
    const noAisle = recipeInputSchema.parse({
      title: 'X',
      baseServings: 2,
      ingredients: [{ quantity: 1, name: 'sel' }],
      steps: ['Saler.'],
    })
    expect(noAisle.ingredients[0]!.aisle).toBe('other')
    expect(noAisle.ingredients[0]!.unit).toBeNull()
    expect(noAisle.freezable).toBe(false)
  })
})
