import type { RecipeInput, RecipeListQuery } from '#shared/schemas/recipe'
import type { RecipeDetail, RecipeSummary, TagSummary } from '#shared/types/recipe'
import { and, asc, count, desc, eq, exists, ilike, inArray, isNull, or, sql } from 'drizzle-orm'
import type { Db } from '../database/client'
import { recipeIngredients, recipes, recipeSteps, recipeTags, tags } from '../database/schema'

// Couche d'accès unique aux recettes : l'isolation multi-utilisateur (filtre par
// userId + deletedAt IS NULL) est garantie ICI, jamais réimplémentée dans les
// handlers (PRD §6/§8). Toute nouvelle requête recette passe par ce module.

const LIST_LIMIT = 60

function ownedActive(userId: string) {
  return and(eq(recipes.userId, userId), isNull(recipes.deletedAt))
}

type RecipeRow = typeof recipes.$inferSelect & {
  recipeTags: Array<{ tag: { name: string } }>
}

export function toRecipeSummary(row: RecipeRow): RecipeSummary {
  return {
    id: row.id,
    title: row.title,
    imageUrl: row.imageUrl,
    baseServings: row.baseServings,
    prepMinutes: row.prepMinutes,
    cookMinutes: row.cookMinutes,
    freezable: row.freezable,
    tags: row.recipeTags.map(rt => rt.tag.name).sort((a, b) => a.localeCompare(b)),
  }
}

export async function listRecipes(db: Db, userId: string, query: RecipeListQuery = {}): Promise<RecipeSummary[]> {
  const conditions = [ownedActive(userId)]

  if (query.q) {
    const pattern = `%${query.q.replaceAll('%', '\\%').replaceAll('_', '\\_')}%`
    conditions.push(or(
      ilike(recipes.title, pattern),
      exists(
        db.select({ one: sql`1` }).from(recipeIngredients)
          .where(and(eq(recipeIngredients.recipeId, recipes.id), ilike(recipeIngredients.name, pattern))),
      ),
    ))
  }

  if (query.tag) {
    conditions.push(inArray(
      recipes.id,
      db.select({ id: recipeTags.recipeId }).from(recipeTags)
        .innerJoin(tags, eq(tags.id, recipeTags.tagId))
        .where(and(eq(tags.userId, userId), eq(tags.name, query.tag))),
    ))
  }

  if (query.maxMinutes) {
    conditions.push(sql`coalesce(${recipes.prepMinutes}, 0) + coalesce(${recipes.cookMinutes}, 0) <= ${query.maxMinutes}`)
  }

  const rows = await db.query.recipes.findMany({
    where: and(...conditions),
    with: { recipeTags: { with: { tag: { columns: { name: true } } } } },
    orderBy: desc(recipes.createdAt),
    limit: LIST_LIMIT,
  })

  return rows.map(toRecipeSummary)
}

export async function getRecipe(db: Db, userId: string, recipeId: string): Promise<RecipeDetail | null> {
  const row = await db.query.recipes.findFirst({
    where: and(eq(recipes.id, recipeId), ownedActive(userId)),
    with: {
      steps: { orderBy: asc(recipeSteps.position) },
      ingredients: { orderBy: asc(recipeIngredients.position) },
      recipeTags: { with: { tag: { columns: { name: true } } } },
    },
  })
  if (!row) return null

  return {
    ...toRecipeSummary(row),
    description: row.description,
    sourceUrl: row.sourceUrl,
    notes: row.notes,
    createdAt: row.createdAt.toISOString(),
    updatedAt: row.updatedAt.toISOString(),
    steps: row.steps.map(s => ({ id: s.id, position: s.position, text: s.text })),
    ingredients: row.ingredients.map(i => ({
      id: i.id,
      position: i.position,
      quantity: Number(i.quantity),
      unit: i.unit,
      name: i.name,
      aisle: i.aisle,
    })),
  }
}

export async function createRecipe(db: Db, userId: string, input: RecipeInput): Promise<RecipeDetail> {
  const recipeId = await db.transaction(async (tx) => {
    const [recipe] = await tx.insert(recipes).values({
      userId,
      title: input.title,
      description: input.description ?? null,
      imageUrl: input.imageUrl ?? null,
      baseServings: input.baseServings,
      prepMinutes: input.prepMinutes ?? null,
      cookMinutes: input.cookMinutes ?? null,
      sourceUrl: input.sourceUrl ?? null,
      freezable: input.freezable,
      notes: input.notes ?? null,
    }).returning({ id: recipes.id })

    await insertChildren(tx, userId, recipe!.id, input)
    return recipe!.id
  })

  return (await getRecipe(db, userId, recipeId))!
}

export async function updateRecipe(db: Db, userId: string, recipeId: string, input: RecipeInput): Promise<RecipeDetail | null> {
  const updated = await db.transaction(async (tx) => {
    const [recipe] = await tx.update(recipes).set({
      title: input.title,
      description: input.description ?? null,
      imageUrl: input.imageUrl ?? null,
      baseServings: input.baseServings,
      prepMinutes: input.prepMinutes ?? null,
      cookMinutes: input.cookMinutes ?? null,
      sourceUrl: input.sourceUrl ?? null,
      freezable: input.freezable,
      notes: input.notes ?? null,
      updatedAt: new Date(),
    }).where(and(eq(recipes.id, recipeId), ownedActive(userId)))
      .returning({ id: recipes.id })

    if (!recipe) return false

    await tx.delete(recipeSteps).where(eq(recipeSteps.recipeId, recipeId))
    await tx.delete(recipeIngredients).where(eq(recipeIngredients.recipeId, recipeId))
    await tx.delete(recipeTags).where(eq(recipeTags.recipeId, recipeId))
    await insertChildren(tx, userId, recipeId, input)
    return true
  })

  return updated ? getRecipe(db, userId, recipeId) : null
}

export async function softDeleteRecipe(db: Db, userId: string, recipeId: string): Promise<boolean> {
  const [row] = await db.update(recipes)
    .set({ deletedAt: new Date(), updatedAt: new Date() })
    .where(and(eq(recipes.id, recipeId), ownedActive(userId)))
    .returning({ id: recipes.id })
  return Boolean(row)
}

export async function listTags(db: Db, userId: string): Promise<TagSummary[]> {
  const rows = await db.select({ name: tags.name, recipeCount: count(recipes.id) })
    .from(tags)
    .innerJoin(recipeTags, eq(recipeTags.tagId, tags.id))
    .innerJoin(recipes, and(eq(recipes.id, recipeTags.recipeId), isNull(recipes.deletedAt)))
    .where(eq(tags.userId, userId))
    .groupBy(tags.name)
    .orderBy(desc(count(recipes.id)), asc(tags.name))
  return rows.map(r => ({ name: r.name, recipeCount: Number(r.recipeCount) }))
}

type Tx = Parameters<Parameters<Db['transaction']>[0]>[0]

async function insertChildren(tx: Tx, userId: string, recipeId: string, input: RecipeInput) {
  await tx.insert(recipeSteps).values(
    input.steps.map((text, index) => ({ recipeId, position: index + 1, text })),
  )
  await tx.insert(recipeIngredients).values(
    input.ingredients.map((ing, index) => ({
      recipeId,
      position: index + 1,
      quantity: ing.quantity.toFixed(2),
      unit: ing.unit,
      name: ing.name,
      aisle: ing.aisle,
    })),
  )
  if (input.tags.length) {
    await tx.insert(tags).values(input.tags.map(name => ({ userId, name }))).onConflictDoNothing()
    const tagRows = await tx.select({ id: tags.id }).from(tags)
      .where(and(eq(tags.userId, userId), inArray(tags.name, input.tags)))
    await tx.insert(recipeTags).values(tagRows.map(t => ({ recipeId, tagId: t.id })))
  }
}
