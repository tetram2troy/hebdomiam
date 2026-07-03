import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { eq } from 'drizzle-orm'
import { recipeInputSchema } from '../../shared/schemas/recipe'
import { mealPlanRecipes, mealPlans } from '../../server/database/schema'
import {
  createPlan,
  getCurrentPlanBundle,
  getPlan,
  listReplacementOptions,
  replacePlanRecipe,
  swapPlanRecipe,
  updatePlanRecipeStatus,
  validatePlan,
} from '../../server/utils/plan-repo'
import { createRecipe } from '../../server/utils/recipe-repo'
import { connectTestDb, createTestUser, truncateAll } from './test-db'

const db = connectTestDb()
const NOW = new Date('2026-07-03T12:00:00') // vendredi → semaine du 2026-06-29

function recipeInput(title: string, tags: string[] = []) {
  return recipeInputSchema.parse({
    title,
    baseServings: 4,
    ingredients: [{ quantity: 1, unit: 'unit', name: `ingrédient ${title}` }],
    steps: [`Cuisiner ${title}.`],
    tags,
  })
}

async function seedRecipes(userId: string, titles: Array<[string, string[]?]>) {
  const created = []
  for (const [title, tags] of titles) {
    created.push(await createRecipe(db, userId, recipeInput(title, tags ?? [])))
  }
  return created
}

const PARAMS = { dishCount: 4, peopleCount: 1, daysCount: 7, replace: false }

beforeEach(async () => {
  await truncateAll(db)
})

afterAll(async () => {
  await db.$client.end()
})

describe('createPlan', () => {
  it('crée un brouillon daté du lundi avec N suggestions et les portions calculées', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D'], ['E']])

    const result = await createPlan(db, user.id, PARAMS, NOW)
    expect('plan' in result).toBe(true)
    const plan = ('plan' in result && result.plan)!

    expect(plan.weekStartDate).toBe('2026-06-29')
    expect(plan.status).toBe('draft')
    expect(plan.totalServings).toBe(14) // 7 j × 2 repas × 1 personne
    expect(plan.recipes).toHaveLength(4)
    expect(plan.recipes.every(r => r.status === 'suggested')).toBe(true)
    expect(plan.recipes.every(r => r.targetServings === 4)).toBe(true) // ceil(14/4)
    expect(plan.recipes.map(r => r.position)).toEqual([1, 2, 3, 4])
  })

  it('refuse si la collection est plus petite que le nombre de plats', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B']])
    expect(await createPlan(db, user.id, PARAMS, NOW)).toEqual({ error: 'notEnoughRecipes' })
  })

  it('refuse un second plan la même semaine, sauf en mode remplacement', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D']])

    const first = await createPlan(db, user.id, PARAMS, NOW)
    const firstId = ('plan' in first && first.plan.id) as string
    expect(await createPlan(db, user.id, PARAMS, NOW)).toEqual({ error: 'planExists' })

    const replaced = await createPlan(db, user.id, { ...PARAMS, dishCount: 3, replace: true }, NOW)
    expect('plan' in replaced && replaced.plan.dishCount).toBe(3)
    expect(await getPlan(db, user.id, firstId)).toBeNull() // l'ancien est parti
  })

  it('applique la rotation : une recette cuisinée récemment n\'est pas suggérée', async () => {
    const user = await createTestUser(db)
    const [cooked] = await seedRecipes(user.id, [['Cuisinée hier'], ['B'], ['C'], ['D'], ['E']])

    // Historique : plan de la semaine passée, recette cuisinée hier
    const lastWeek = await createPlan(db, user.id, { ...PARAMS, dishCount: 1 }, new Date('2026-06-26T12:00:00'))
    const lastPlanId = ('plan' in lastWeek && lastWeek.plan.id) as string
    await db.update(mealPlans).set({ status: 'validated' }).where(eq(mealPlans.id, lastPlanId))
    await db.update(mealPlanRecipes)
      .set({ recipeId: cooked!.id, status: 'cooked', cookedAt: new Date('2026-07-02T18:00:00') })
      .where(eq(mealPlanRecipes.mealPlanId, lastPlanId))

    const result = await createPlan(db, user.id, PARAMS, NOW)
    const plan = ('plan' in result && result.plan)!
    expect(plan.recipes.map(r => r.recipe.title)).not.toContain('Cuisinée hier')
  })
})

describe('cycle valider / échanger / remplacer / cuisiner', () => {
  it('accepte puis annule une suggestion', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D']])
    const { plan } = (await createPlan(db, user.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    const item = plan.recipes[0]!
    const accepted = await updatePlanRecipeStatus(db, user.id, plan.id, item.id, 'accept')
    expect('plan' in accepted && accepted.plan.recipes.find(r => r.id === item.id)!.status).toBe('accepted')

    const undone = await updatePlanRecipeStatus(db, user.id, plan.id, item.id, 'unaccept')
    expect('plan' in undone && undone.plan.recipes.find(r => r.id === item.id)!.status).toBe('suggested')
  })

  it('échange : la carte est remplacée par une recette jamais proposée dans ce plan', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D'], ['E']])
    const { plan } = (await createPlan(db, user.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    const before = plan.recipes.map(r => r.recipe.id)
    const item = plan.recipes[0]!
    const swapped = await swapPlanRecipe(db, user.id, plan.id, item.id, NOW)
    const after = ('plan' in swapped && swapped.plan)!

    expect(after.recipes).toHaveLength(4)
    const newItem = after.recipes.find(r => r.position === item.position)!
    expect(before).not.toContain(newItem.recipe.id)
    expect(newItem.status).toBe('suggested')
  })

  it('échange épuisé : refuse proprement quand il n\'y a plus de candidat', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D']]) // aucun surplus
    const { plan } = (await createPlan(db, user.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    expect(await swapPlanRecipe(db, user.id, plan.id, plan.recipes[0]!.id, NOW))
      .toEqual({ error: 'noMoreSuggestions' })
  })

  it('remplacement manuel : la recette choisie arrive déjà acceptée', async () => {
    const user = await createTestUser(db)
    const created = await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D'], ['Choix précis']])
    const { plan } = (await createPlan(db, user.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    const target = created.find(r => r.title === 'Choix précis')!
    const inPlan = plan.recipes.some(r => r.recipe.id === target.id)
    const item = plan.recipes.find(r => r.recipe.id !== target.id)!

    if (inPlan) {
      // la cible est déjà au plan → remplacement interdit (invalidState)
      expect(await replacePlanRecipe(db, user.id, plan.id, item.id, target.id))
        .toEqual({ error: 'invalidState' })
    }
    else {
      const replaced = await replacePlanRecipe(db, user.id, plan.id, item.id, target.id)
      const after = ('plan' in replaced && replaced.plan)!
      const newItem = after.recipes.find(r => r.recipe.id === target.id)!
      expect(newItem.status).toBe('accepted')
      expect(newItem.position).toBe(item.position)
    }
  })

  it('valide le plan (les suggestions restantes deviennent acceptées) puis cuisine tout', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D']])
    const { plan } = (await createPlan(db, user.id, { ...PARAMS, dishCount: 2 }, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    const validated = ('plan' in (await validatePlan(db, user.id, plan.id))
      && (await getPlan(db, user.id, plan.id)))!
    expect(validated.status).toBe('validated')
    expect(validated.recipes.every(r => r.status === 'accepted')).toBe(true)

    // cuisiner la première : plan reste validé ; la seconde : plan devient cooked
    await updatePlanRecipeStatus(db, user.id, plan.id, validated.recipes[0]!.id, 'cook', NOW)
    let current = (await getPlan(db, user.id, plan.id))!
    expect(current.status).toBe('validated')
    expect(current.recipes[0]!.cookedAt).not.toBeNull()

    await updatePlanRecipeStatus(db, user.id, plan.id, validated.recipes[1]!.id, 'cook', NOW)
    current = (await getPlan(db, user.id, plan.id))!
    expect(current.status).toBe('cooked')
  })

  it('interdit de cuisiner sur un brouillon et de valider un plan déjà validé', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D']])
    const { plan } = (await createPlan(db, user.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    expect(await updatePlanRecipeStatus(db, user.id, plan.id, plan.recipes[0]!.id, 'cook'))
      .toEqual({ error: 'invalidState' })

    await validatePlan(db, user.id, plan.id)
    expect(await validatePlan(db, user.id, plan.id)).toEqual({ error: 'invalidState' })
  })
})

describe('options de remplacement', () => {
  it('mode recherche : exclut les recettes déjà actives au plan et annote la rotation', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D'], ['Hors plan']])
    const { plan } = (await createPlan(db, user.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    const options = await listReplacementOptions(db, user.id, plan.id, { mode: 'search' }, NOW)
    expect(Array.isArray(options)).toBe(true)
    const list = options as Exclude<typeof options, { error: unknown }>
    expect(list).toHaveLength(1)
    expect(list[0]!.cookedCount).toBe(0)
    expect(list[0]!.lastCookedAt).toBeNull()
    const planIds = new Set(plan.recipes.map(r => r.recipe.id))
    expect(planIds.has(list[0]!.id)).toBe(false)
  })

  it('mode suggestion : petit lot hors de tout ce que le plan a touché', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D'], ['E'], ['F'], ['G'], ['H']])
    const { plan } = (await createPlan(db, user.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    const options = await listReplacementOptions(db, user.id, plan.id, { mode: 'suggest' }, NOW)
    const list = options as Exclude<typeof options, { error: unknown }>
    expect(list).toHaveLength(3)
    const planIds = new Set(plan.recipes.map(r => r.recipe.id))
    expect(list.every(o => !planIds.has(o.id))).toBe(true)
  })
})

describe('getCurrentPlanBundle', () => {
  it('retourne le plan de la semaine courante et les défauts utilisateur', async () => {
    const user = await createTestUser(db)
    await seedRecipes(user.id, [['A'], ['B'], ['C'], ['D']])

    const empty = await getCurrentPlanBundle(db, user.id, NOW)
    expect(empty.plan).toBeNull()
    expect(empty.defaults).toEqual({
      dishCount: 4, peopleCount: 1, daysCount: 7, rotationWeeks: 3, recipeCount: 4,
    })

    await createPlan(db, user.id, PARAMS, NOW)
    const bundle = await getCurrentPlanBundle(db, user.id, NOW)
    expect(bundle.plan?.weekStartDate).toBe('2026-06-29')
  })
})

describe('isolation multi-utilisateur (IDOR)', () => {
  it('un autre utilisateur ne peut ni lire ni manipuler le plan', async () => {
    const alice = await createTestUser(db, 'Alice')
    const bob = await createTestUser(db, 'Bob')
    await seedRecipes(alice.id, [['A'], ['B'], ['C'], ['D']])
    const { plan } = (await createPlan(db, alice.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }
    const item = plan.recipes[0]!

    expect(await getPlan(db, bob.id, plan.id)).toBeNull()
    expect(await updatePlanRecipeStatus(db, bob.id, plan.id, item.id, 'accept')).toEqual({ error: 'notFound' })
    expect(await swapPlanRecipe(db, bob.id, plan.id, item.id)).toEqual({ error: 'notFound' })
    expect(await validatePlan(db, bob.id, plan.id)).toEqual({ error: 'notFound' })
    expect(await listReplacementOptions(db, bob.id, plan.id, { mode: 'search' })).toEqual({ error: 'notFound' })

    // le plan d'Alice est intact
    const intact = (await getPlan(db, alice.id, plan.id))!
    expect(intact.recipes[0]!.status).toBe('suggested')
  })

  it('le remplacement manuel ne peut pas pointer la recette d\'un autre compte', async () => {
    const alice = await createTestUser(db, 'Alice')
    const bob = await createTestUser(db, 'Bob')
    await seedRecipes(alice.id, [['A'], ['B'], ['C'], ['D']])
    const [bobRecipe] = await seedRecipes(bob.id, [['Recette de Bob']])
    const { plan } = (await createPlan(db, alice.id, PARAMS, NOW)) as { plan: NonNullable<Awaited<ReturnType<typeof getPlan>>> }

    expect(await replacePlanRecipe(db, alice.id, plan.id, plan.recipes[0]!.id, bobRecipe!.id))
      .toEqual({ error: 'notFound' })
  })
})
