import type { PlanCreateInput, ReplacementQuery } from '#shared/schemas/plan'
import type { MealPlanDto, PlanDefaults, PlanRecipeDto, ReplacementOption } from '#shared/types/plan'
import { and, count, eq, isNull, max } from 'drizzle-orm'
import type { Db } from '../database/client'
import { mealPlanRecipes, mealPlans, recipes, users } from '../database/schema'
import { generateGroceryList } from './grocery-repo'
import { listRecipes, toRecipeSummary } from './recipe-repo'
import { computePortions, mondayOf, pickSuggestions, type RotationCandidate } from './rotation'

// Couche d'accès unique aux plans hebdomadaires : l'isolation multi-utilisateur
// est garantie ICI (filtre par userId sur chaque requête). Les fonctions
// retournent un code d'erreur métier ; les routes le traduisent en HTTP.

export type PlanError = 'notFound' | 'planExists' | 'notEnoughRecipes' | 'noMoreSuggestions' | 'invalidState'
export type PlanResult = { plan: MealPlanDto } | { error: PlanError }

export const PLAN_ERROR_STATUS: Record<PlanError, number> = {
  notFound: 404,
  planExists: 409,
  notEnoughRecipes: 409,
  noMoreSuggestions: 409,
  invalidState: 409,
}

const PLAN_WITH = {
  planRecipes: {
    with: {
      recipe: { with: { recipeTags: { with: { tag: { columns: { name: true } } } } } },
    },
  },
} as const

type PlanRow = typeof mealPlans.$inferSelect & {
  planRecipes: Array<typeof mealPlanRecipes.$inferSelect & {
    recipe: Parameters<typeof toRecipeSummary>[0]
  }>
}

function toPlanDto(row: PlanRow): MealPlanDto {
  const active = row.planRecipes
    .filter(pr => pr.status !== 'swapped')
    .sort((a, b) => a.position - b.position)
  return {
    id: row.id,
    weekStartDate: row.weekStartDate,
    peopleCount: row.peopleCount,
    dishCount: row.dishCount,
    daysCount: row.daysCount,
    totalServings: row.totalServings,
    status: row.status,
    recipes: active.map(pr => ({
      id: pr.id,
      position: pr.position,
      status: pr.status as PlanRecipeDto['status'],
      targetServings: pr.targetServings,
      cookedAt: pr.cookedAt?.toISOString() ?? null,
      recipe: toRecipeSummary(pr.recipe),
    })),
  }
}

/** Dernière cuisson et nombre de cuissons par recette (source : historique des plans). */
async function getCookHistory(db: Db, userId: string) {
  const rows = await db.select({
    recipeId: mealPlanRecipes.recipeId,
    lastCookedAt: max(mealPlanRecipes.cookedAt),
    cookedCount: count(mealPlanRecipes.id),
  })
    .from(mealPlanRecipes)
    .innerJoin(mealPlans, eq(mealPlans.id, mealPlanRecipes.mealPlanId))
    .where(and(eq(mealPlans.userId, userId), eq(mealPlanRecipes.status, 'cooked')))
    .groupBy(mealPlanRecipes.recipeId)

  return new Map(rows.map(r => [r.recipeId, {
    lastCookedAt: r.lastCookedAt as Date | null,
    cookedCount: Number(r.cookedCount),
  }]))
}

async function getCandidates(db: Db, userId: string): Promise<RotationCandidate[]> {
  const [rows, history] = await Promise.all([
    db.query.recipes.findMany({
      where: and(eq(recipes.userId, userId), isNull(recipes.deletedAt)),
      with: { recipeTags: { with: { tag: { columns: { name: true } } } } },
    }),
    getCookHistory(db, userId),
  ])
  return rows.map(r => ({
    id: r.id,
    tags: r.recipeTags.map(rt => rt.tag.name),
    lastCookedAt: history.get(r.id)?.lastCookedAt ?? null,
    createdAt: r.createdAt,
  }))
}

async function findPlanRow(db: Db, userId: string, planId: string): Promise<PlanRow | null> {
  const row = await db.query.mealPlans.findFirst({
    where: and(eq(mealPlans.id, planId), eq(mealPlans.userId, userId)),
    with: PLAN_WITH,
  })
  return (row as PlanRow | undefined) ?? null
}

export async function getPlan(db: Db, userId: string, planId: string): Promise<MealPlanDto | null> {
  const row = await findPlanRow(db, userId, planId)
  return row ? toPlanDto(row) : null
}

export async function getCurrentPlanBundle(db: Db, userId: string, now = new Date()) {
  const week = mondayOf(now)
  const [row, user, recipeCount] = await Promise.all([
    db.query.mealPlans.findFirst({
      where: and(eq(mealPlans.userId, userId), eq(mealPlans.weekStartDate, week)),
      with: PLAN_WITH,
    }),
    db.query.users.findFirst({ where: eq(users.id, userId) }),
    db.select({ n: count() }).from(recipes)
      .where(and(eq(recipes.userId, userId), isNull(recipes.deletedAt))),
  ])

  const defaults: PlanDefaults = {
    dishCount: user?.defaultDishCount ?? 4,
    peopleCount: user?.defaultPeopleCount ?? 1,
    daysCount: user?.defaultDaysCount ?? 7,
    rotationWeeks: user?.rotationWeeks ?? 3,
    recipeCount: Number(recipeCount[0]?.n ?? 0),
  }

  return { plan: row ? toPlanDto(row as PlanRow) : null, defaults }
}

export async function createPlan(db: Db, userId: string, input: PlanCreateInput, now = new Date()): Promise<PlanResult> {
  const week = mondayOf(now)
  const existing = await db.query.mealPlans.findFirst({
    where: and(eq(mealPlans.userId, userId), eq(mealPlans.weekStartDate, week)),
    columns: { id: true },
  })
  if (existing && !input.replace) return { error: 'planExists' }

  const candidates = await getCandidates(db, userId)
  if (candidates.length < input.dishCount) return { error: 'notEnoughRecipes' }

  const user = await db.query.users.findFirst({ where: eq(users.id, userId) })
  const { totalServings, servingsPerDish } = computePortions(input)
  const suggestionIds = pickSuggestions(candidates, {
    count: input.dishCount,
    rotationWeeks: user?.rotationWeeks ?? 3,
    now,
  })

  const planId = await db.transaction(async (tx) => {
    if (existing) {
      await tx.delete(mealPlans).where(eq(mealPlans.id, existing.id))
    }
    const [plan] = await tx.insert(mealPlans).values({
      userId,
      weekStartDate: week,
      peopleCount: input.peopleCount,
      dishCount: input.dishCount,
      daysCount: input.daysCount,
      totalServings,
    }).returning({ id: mealPlans.id })

    await tx.insert(mealPlanRecipes).values(suggestionIds.map((recipeId, index) => ({
      mealPlanId: plan!.id,
      recipeId,
      position: index + 1,
      targetServings: servingsPerDish,
    })))
    return plan!.id
  })

  return { plan: (await getPlan(db, userId, planId))! }
}

export async function updatePlanRecipeStatus(db: Db, userId: string, planId: string, itemId: string, action: 'accept' | 'unaccept' | 'cook', now = new Date()): Promise<PlanResult> {
  const row = await findPlanRow(db, userId, planId)
  if (!row) return { error: 'notFound' }
  const item = row.planRecipes.find(pr => pr.id === itemId)
  if (!item) return { error: 'notFound' }

  const allowed
    = (action === 'accept' && row.status === 'draft' && item.status === 'suggested')
      || (action === 'unaccept' && row.status === 'draft' && item.status === 'accepted')
      || (action === 'cook' && row.status === 'validated' && item.status === 'accepted')
  if (!allowed) return { error: 'invalidState' }

  await db.transaction(async (tx) => {
    if (action === 'cook') {
      await tx.update(mealPlanRecipes)
        .set({ status: 'cooked', cookedAt: now })
        .where(eq(mealPlanRecipes.id, itemId))
      const stillAccepted = row.planRecipes.some(pr => pr.id !== itemId && pr.status === 'accepted')
      if (!stillAccepted) {
        await tx.update(mealPlans).set({ status: 'cooked' }).where(eq(mealPlans.id, planId))
      }
    }
    else {
      await tx.update(mealPlanRecipes)
        .set({ status: action === 'accept' ? 'accepted' : 'suggested' })
        .where(eq(mealPlanRecipes.id, itemId))
    }
  })

  return { plan: (await getPlan(db, userId, planId))! }
}

export async function swapPlanRecipe(db: Db, userId: string, planId: string, itemId: string, now = new Date()): Promise<PlanResult> {
  const row = await findPlanRow(db, userId, planId)
  if (!row) return { error: 'notFound' }
  const item = row.planRecipes.find(pr => pr.id === itemId)
  if (!item) return { error: 'notFound' }
  if (row.status !== 'draft' || (item.status !== 'suggested' && item.status !== 'accepted')) {
    return { error: 'invalidState' }
  }

  // Mémoire du plan : tout ce qui y est ou y a été (même écarté) est exclu.
  const touchedIds = new Set(row.planRecipes.map(pr => pr.recipeId))
  const user = await db.query.users.findFirst({ where: eq(users.id, userId) })
  const candidates = (await getCandidates(db, userId)).filter(c => !touchedIds.has(c.id))
  const [newRecipeId] = pickSuggestions(candidates, {
    count: 1,
    rotationWeeks: user?.rotationWeeks ?? 3,
    now,
  })
  if (!newRecipeId) return { error: 'noMoreSuggestions' }

  await db.transaction(async (tx) => {
    await tx.update(mealPlanRecipes).set({ status: 'swapped' }).where(eq(mealPlanRecipes.id, itemId))
    await tx.insert(mealPlanRecipes).values({
      mealPlanId: planId,
      recipeId: newRecipeId,
      position: item.position,
      targetServings: item.targetServings,
    })
  })

  return { plan: (await getPlan(db, userId, planId))! }
}

export async function replacePlanRecipe(db: Db, userId: string, planId: string, itemId: string, recipeId: string): Promise<PlanResult> {
  const row = await findPlanRow(db, userId, planId)
  if (!row) return { error: 'notFound' }
  const item = row.planRecipes.find(pr => pr.id === itemId)
  if (!item) return { error: 'notFound' }
  if (row.status !== 'draft' || (item.status !== 'suggested' && item.status !== 'accepted')) {
    return { error: 'invalidState' }
  }

  const replacement = await db.query.recipes.findFirst({
    where: and(eq(recipes.id, recipeId), eq(recipes.userId, userId), isNull(recipes.deletedAt)),
    columns: { id: true },
  })
  if (!replacement) return { error: 'notFound' }

  const activeIds = row.planRecipes.filter(pr => pr.status !== 'swapped').map(pr => pr.recipeId)
  if (activeIds.includes(recipeId)) return { error: 'invalidState' }

  await db.transaction(async (tx) => {
    await tx.update(mealPlanRecipes).set({ status: 'swapped' }).where(eq(mealPlanRecipes.id, itemId))
    // Choix manuel = intention claire : la rangée arrive déjà validée.
    await tx.insert(mealPlanRecipes).values({
      mealPlanId: planId,
      recipeId,
      position: item.position,
      targetServings: item.targetServings,
      status: 'accepted',
    })
  })

  return { plan: (await getPlan(db, userId, planId))! }
}

export async function validatePlan(db: Db, userId: string, planId: string): Promise<PlanResult> {
  const row = await findPlanRow(db, userId, planId)
  if (!row) return { error: 'notFound' }
  if (row.status !== 'draft') return { error: 'invalidState' }
  const active = row.planRecipes.filter(pr => pr.status === 'suggested' || pr.status === 'accepted')
  if (active.length === 0) return { error: 'invalidState' }

  await db.transaction(async (tx) => {
    await tx.update(mealPlanRecipes)
      .set({ status: 'accepted' })
      .where(and(eq(mealPlanRecipes.mealPlanId, planId), eq(mealPlanRecipes.status, 'suggested')))
    await tx.update(mealPlans).set({ status: 'validated' }).where(eq(mealPlans.id, planId))
    // La liste d'épicerie naît avec la validation (PRD F6) — même transaction.
    await generateGroceryList(tx, planId)
  })

  return { plan: (await getPlan(db, userId, planId))! }
}

export async function listReplacementOptions(db: Db, userId: string, planId: string, query: ReplacementQuery, now = new Date()): Promise<ReplacementOption[] | { error: PlanError }> {
  const row = await findPlanRow(db, userId, planId)
  if (!row) return { error: 'notFound' }

  const history = await getCookHistory(db, userId)
  const activeIds = new Set(row.planRecipes.filter(pr => pr.status !== 'swapped').map(pr => pr.recipeId))

  function augment(summaryList: Awaited<ReturnType<typeof listRecipes>>): ReplacementOption[] {
    return summaryList.map(s => ({
      ...s,
      lastCookedAt: history.get(s.id)?.lastCookedAt?.toISOString() ?? null,
      cookedCount: history.get(s.id)?.cookedCount ?? 0,
    }))
  }

  if (query.mode === 'search') {
    const found = await listRecipes(db, userId, { q: query.q })
    return augment(found.filter(s => !activeIds.has(s.id))).slice(0, 20)
  }

  // mode « Me suggérer autre chose » : petit lot choisi par la rotation,
  // en excluant tout ce que le plan a déjà touché (même les écartées).
  const touchedIds = new Set(row.planRecipes.map(pr => pr.recipeId))
  const user = await db.query.users.findFirst({ where: eq(users.id, userId) })
  const candidates = (await getCandidates(db, userId)).filter(c => !touchedIds.has(c.id))
  const ids = pickSuggestions(candidates, { count: 3, rotationWeeks: user?.rotationWeeks ?? 3, now })
  if (ids.length === 0) return []

  const all = await listRecipes(db, userId)
  const byId = new Map(all.map(s => [s.id, s]))
  return augment(ids.map(id => byId.get(id)!).filter(Boolean))
}
