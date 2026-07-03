import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { eq } from 'drizzle-orm'
import { recipeInputSchema, type RecipeInput } from '../../shared/schemas/recipe'
import { users } from '../../server/database/schema'
import {
  addManualGroceryItem,
  deleteGroceryItem,
  getGroceryListForWeek,
  updateGroceryItem,
} from '../../server/utils/grocery-repo'
import { createPlan, validatePlan, type PlanResult } from '../../server/utils/plan-repo'
import { createRecipe } from '../../server/utils/recipe-repo'
import { connectTestDb, createTestUser, truncateAll } from './test-db'

const db = connectTestDb()
const NOW = new Date('2026-07-03T12:00:00') // vendredi → semaine du 2026-06-29

function recipeInput(title: string, ingredients: Array<Partial<RecipeInput['ingredients'][number]> & { name: string, quantity: number }>, baseServings = 4) {
  return recipeInputSchema.parse({
    title,
    baseServings,
    ingredients,
    steps: [`Cuisiner ${title}.`],
  })
}

/** Deux recettes (portions de base 4 et 2) validées avec 2 plats × 7 portions
    → échelles ×1,75 et ×3,5. */
async function seedValidatedPlan(userId: string) {
  await createRecipe(db, userId, recipeInput('Poulet au riz', [
    { name: 'riz basmati', quantity: 200, unit: 'g', aisle: 'dry' },
    { name: 'tomates', quantity: 2, unit: 'unit', aisle: 'produce' },
    { name: 'bouillon de poulet', quantity: 2, unit: 'cup', aisle: 'canned' },
  ]))
  await createRecipe(db, userId, recipeInput('Tofu au riz', [
    { name: 'Riz  Basmati', quantity: 100, unit: 'g', aisle: 'dry' },
    { name: 'tomates', quantity: 400, unit: 'g', aisle: 'produce' },
    { name: 'crème', quantity: 400, unit: 'ml', aisle: 'dairy' },
  ], 2))

  const created = await createPlan(db, userId, { dishCount: 2, peopleCount: 1, daysCount: 7, replace: false }, NOW)
  const planId = (created as Extract<PlanResult, { plan: unknown }>).plan.id
  await validatePlan(db, userId, planId)
  return planId
}

beforeEach(async () => {
  await truncateAll(db)
})

afterAll(async () => {
  await db.$client.end()
})

describe('génération à la validation du plan', () => {
  it('agrège les ingrédients mis à l\'échelle, par nom normalisé et dimension', async () => {
    const user = await createTestUser(db)
    await seedValidatedPlan(user.id)

    const list = (await getGroceryListForWeek(db, user.id, NOW))!
    expect(list).not.toBeNull()
    expect(list.weekStartDate).toBe('2026-06-29')

    // riz : 200 g × 1,75 + 100 g × 3,5 = 700 g — fusion malgré la casse
    const riz = list.items.find(i => i.label.toLowerCase().startsWith('riz'))!
    expect(riz.quantity).toBe(700)
    expect(riz.unit).toBe('g')
    expect(riz.aisle).toBe('dry')
    expect(riz.sources.map(s => s.recipeTitle).sort()).toEqual(['Poulet au riz', 'Tofu au riz'])

    // tomates : unités incompatibles → deux lignes (3,5 unités et 1,4 kg)
    const tomates = list.items.filter(i => i.label === 'tomates')
    expect(tomates).toHaveLength(2)
    const parUnite = tomates.find(t => t.unit === 'unit')!
    const parPoids = tomates.find(t => t.unit === 'kg')!
    expect(parUnite.quantity).toBe(3.5)
    expect(parPoids.quantity).toBe(1.4) // 400 g × 3,5 = 1400 g → 1,4 kg (affichage métrique)

    // volumes : 2 tasses × 1,75 = 875 ml ; crème 400 ml × 3,5 = 1,4 L
    expect(list.items.find(i => i.label === 'bouillon de poulet')).toMatchObject({ quantity: 875, unit: 'ml' })
    expect(list.items.find(i => i.label === 'crème')).toMatchObject({ quantity: 1.4, unit: 'l' })

    // tri : ordre des rayons du magasin (fruits et légumes avant produits secs)
    const aisles = list.items.map(i => i.aisle)
    expect(aisles.indexOf('produce')).toBeLessThan(aisles.indexOf('dry'))
    expect(list.totalCount).toBe(5)
    expect(list.checkedCount).toBe(0)
  })

  it('affiche en impérial quand l\'utilisateur le préfère', async () => {
    const user = await createTestUser(db)
    await db.update(users).set({ unitSystem: 'imperial' }).where(eq(users.id, user.id))
    await seedValidatedPlan(user.id)

    const list = (await getGroceryListForWeek(db, user.id, NOW))!
    const riz = list.items.find(i => i.label.toLowerCase().startsWith('riz'))!
    expect(riz.unit).toBe('lb') // 700 g ≥ 1 lb
    expect(riz.quantity).toBeCloseTo(1.54, 1)
    const bouillon = list.items.find(i => i.label === 'bouillon de poulet')!
    expect(bouillon.unit).toBe('cup') // 875 ml = 3,5 tasses
    expect(bouillon.quantity).toBe(3.5)
  })

  it('pas de liste tant que le plan n\'est pas validé', async () => {
    const user = await createTestUser(db)
    await createRecipe(db, user.id, recipeInput('A', [{ name: 'x', quantity: 1, unit: 'unit' }]))
    await createRecipe(db, user.id, recipeInput('B', [{ name: 'y', quantity: 1, unit: 'unit' }]))
    await createPlan(db, user.id, { dishCount: 2, peopleCount: 1, daysCount: 7, replace: false }, NOW)

    expect(await getGroceryListForWeek(db, user.id, NOW)).toBeNull()
  })
})

describe('interactions en magasin', () => {
  it('coche puis décoche un article (le compteur suit)', async () => {
    const user = await createTestUser(db)
    await seedValidatedPlan(user.id)
    const list = (await getGroceryListForWeek(db, user.id, NOW))!
    const item = list.items[0]!

    const checked = (await updateGroceryItem(db, user.id, item.id, { checked: true }, NOW))!
    expect(checked.checkedCount).toBe(1)
    expect(checked.items.find(i => i.id === item.id)!.checked).toBe(true)

    const unchecked = (await updateGroceryItem(db, user.id, item.id, { checked: false }, NOW))!
    expect(unchecked.checkedCount).toBe(0)
  })

  it('corrige le rayon d\'un article', async () => {
    const user = await createTestUser(db)
    await seedValidatedPlan(user.id)
    const list = (await getGroceryListForWeek(db, user.id, NOW))!
    const item = list.items.find(i => i.label === 'crème')!

    const updated = (await updateGroceryItem(db, user.id, item.id, { aisle: 'frozen' }, NOW))!
    expect(updated.items.find(i => i.id === item.id)!.aisle).toBe('frozen')
  })

  it('ajoute un article libre (sans quantité) puis le retire', async () => {
    const user = await createTestUser(db)
    await seedValidatedPlan(user.id)

    const withManual = (await addManualGroceryItem(db, user.id, { label: 'Essuie-tout', aisle: 'household' }, NOW))!
    const manual = withManual.items.find(i => i.label === 'Essuie-tout')!
    expect(manual.isManual).toBe(true)
    expect(manual.quantity).toBeNull()
    expect(withManual.totalCount).toBe(6)

    const without = (await deleteGroceryItem(db, user.id, manual.id, NOW))!
    expect(without.totalCount).toBe(5)
  })

  it('convertit la quantité corrigée vers l\'unité de base', async () => {
    const user = await createTestUser(db)
    await seedValidatedPlan(user.id)
    const list = (await getGroceryListForWeek(db, user.id, NOW))!
    const riz = list.items.find(i => i.label.toLowerCase().startsWith('riz'))!

    // correction : « finalement 1 kg »
    const updated = (await updateGroceryItem(db, user.id, riz.id, { quantity: 1, unit: 'kg' }, NOW))!
    expect(updated.items.find(i => i.id === riz.id)).toMatchObject({ quantity: 1, unit: 'kg' })
  })
})

describe('isolation multi-utilisateur (IDOR)', () => {
  it('un autre utilisateur ne voit ni ne modifie la liste', async () => {
    const alice = await createTestUser(db, 'Alice')
    const bob = await createTestUser(db, 'Bob')
    await seedValidatedPlan(alice.id)
    const list = (await getGroceryListForWeek(db, alice.id, NOW))!
    const item = list.items[0]!

    expect(await getGroceryListForWeek(db, bob.id, NOW)).toBeNull()
    expect(await updateGroceryItem(db, bob.id, item.id, { checked: true }, NOW)).toBeNull()
    expect(await deleteGroceryItem(db, bob.id, item.id, NOW)).toBeNull()
    expect(await addManualGroceryItem(db, bob.id, { label: 'intrus', aisle: 'other' }, NOW)).toBeNull()

    // rien n'a bougé chez Alice
    const intact = (await getGroceryListForWeek(db, alice.id, NOW))!
    expect(intact.checkedCount).toBe(0)
    expect(intact.totalCount).toBe(5)
  })
})
