import { AISLES } from '#shared/schemas/recipe'
import type { GroceryItemCreateInput, GroceryItemUpdateInput } from '#shared/schemas/grocery'
import type { GroceryListDto } from '#shared/types/grocery'
import { and, eq, inArray } from 'drizzle-orm'
import type { Db } from '../database/client'
import { groceryItems, groceryLists, mealPlanRecipes, mealPlans, users } from '../database/schema'
import { aggregateIngredients, type AggregationInput } from './grocery-aggregate'
import { mondayOf } from './rotation'
import { baseUnitOf, dimensionOf, displayQuantity, toBase } from './units'

// Couche d'accès unique à la liste d'épicerie : l'isolation multi-utilisateur
// passe par la chaîne groceryItems → groceryLists → mealPlans.userId.

type Tx = Parameters<Parameters<Db['transaction']>[0]>[0]

/** Génère la liste agrégée d'un plan — appelée DANS la transaction de
    validation du plan (plan-repo.validatePlan). */
export async function generateGroceryList(tx: Tx, planId: string) {
  const rows = await tx.query.mealPlanRecipes.findMany({
    where: and(
      eq(mealPlanRecipes.mealPlanId, planId),
      inArray(mealPlanRecipes.status, ['accepted', 'cooked']),
    ),
    with: { recipe: { with: { ingredients: true } } },
  })

  const inputs: AggregationInput[] = rows.flatMap(planRecipe =>
    planRecipe.recipe.ingredients.map(ingredient => ({
      ingredientId: ingredient.id,
      name: ingredient.name,
      quantity: Number(ingredient.quantity),
      unit: ingredient.unit,
      aisle: ingredient.aisle,
      recipeTitle: planRecipe.recipe.title,
      baseServings: planRecipe.recipe.baseServings,
      targetServings: planRecipe.targetServings,
    })),
  )

  const [list] = await tx.insert(groceryLists).values({ mealPlanId: planId }).returning({ id: groceryLists.id })
  const aggregated = aggregateIngredients(inputs)
  if (aggregated.length) {
    await tx.insert(groceryItems).values(aggregated.map(item => ({
      groceryListId: list!.id,
      label: item.label,
      quantity: item.quantity.toFixed(2),
      unit: item.unit,
      aisle: item.aisle,
      sources: item.sources,
    })))
  }
}

const AISLE_ORDER = new Map(AISLES.map((aisle, index) => [aisle, index]))

type ListRow = typeof groceryLists.$inferSelect & {
  items: Array<typeof groceryItems.$inferSelect>
  mealPlan: { id: string, weekStartDate: string }
}

function toListDto(row: ListRow, unitSystem: 'metric' | 'imperial'): GroceryListDto {
  const items = row.items
    .map((item) => {
      const display = item.quantity === null
        ? null
        : displayQuantity(Number(item.quantity), item.unit, unitSystem)
      return {
        id: item.id,
        label: item.label,
        quantity: display?.quantity ?? null,
        unit: display ? display.unit : item.unit,
        aisle: item.aisle,
        checked: item.checked,
        isManual: item.isManual,
        sources: (item.sources ?? []).map(s => ({
          recipeTitle: s.recipeTitle,
          quantity: s.quantity,
          unit: s.unit as GroceryListDto['items'][number]['unit'],
        })),
      }
    })
    .sort((a, b) =>
      (AISLE_ORDER.get(a.aisle)! - AISLE_ORDER.get(b.aisle)!)
      || a.label.localeCompare(b.label),
    )

  return {
    id: row.id,
    planId: row.mealPlan.id,
    weekStartDate: row.mealPlan.weekStartDate,
    checkedCount: items.filter(i => i.checked).length,
    totalCount: items.length,
    items,
  }
}

async function findListRowForWeek(db: Db, userId: string, now: Date): Promise<ListRow | null> {
  const plan = await db.query.mealPlans.findFirst({
    where: and(eq(mealPlans.userId, userId), eq(mealPlans.weekStartDate, mondayOf(now))),
    columns: { id: true, weekStartDate: true },
  })
  if (!plan) return null
  const list = await db.query.groceryLists.findFirst({
    where: eq(groceryLists.mealPlanId, plan.id),
    with: { items: true },
  })
  if (!list) return null
  return { ...list, mealPlan: plan }
}

async function getUnitSystem(db: Db, userId: string): Promise<'metric' | 'imperial'> {
  const user = await db.query.users.findFirst({
    where: eq(users.id, userId),
    columns: { unitSystem: true },
  })
  return user?.unitSystem ?? 'metric'
}

export async function getGroceryListForWeek(db: Db, userId: string, now = new Date()): Promise<GroceryListDto | null> {
  const row = await findListRowForWeek(db, userId, now)
  if (!row) return null
  return toListDto(row, await getUnitSystem(db, userId))
}

/** Vérifie que l'article appartient bien à une liste de l'utilisateur. */
async function ownsItem(db: Db, userId: string, itemId: string): Promise<boolean> {
  const [row] = await db.select({ id: groceryItems.id })
    .from(groceryItems)
    .innerJoin(groceryLists, eq(groceryLists.id, groceryItems.groceryListId))
    .innerJoin(mealPlans, eq(mealPlans.id, groceryLists.mealPlanId))
    .where(and(eq(groceryItems.id, itemId), eq(mealPlans.userId, userId)))
  return Boolean(row)
}

export async function updateGroceryItem(db: Db, userId: string, itemId: string, patch: GroceryItemUpdateInput, now = new Date()): Promise<GroceryListDto | null> {
  if (!(await ownsItem(db, userId, itemId))) return null

  const values: Partial<typeof groceryItems.$inferInsert> = {}
  if (patch.checked !== undefined) values.checked = patch.checked
  if (patch.aisle !== undefined) values.aisle = patch.aisle
  if (patch.label !== undefined) values.label = patch.label
  if (patch.quantity !== undefined) {
    // Quantité corrigée fournie dans l'unité saisie → stockée en unité de base.
    values.quantity = patch.quantity === null ? null : toBase(patch.quantity, patch.unit ?? null).toFixed(2)
    values.unit = patch.quantity === null ? null : baseUnitOf(dimensionOf(patch.unit ?? null))
  }
  await db.update(groceryItems).set(values).where(eq(groceryItems.id, itemId))

  return getGroceryListForWeek(db, userId, now)
}

export async function addManualGroceryItem(db: Db, userId: string, input: GroceryItemCreateInput, now = new Date()): Promise<GroceryListDto | null> {
  const row = await findListRowForWeek(db, userId, now)
  if (!row) return null

  await db.insert(groceryItems).values({
    groceryListId: row.id,
    label: input.label,
    quantity: input.quantity == null ? null : toBase(input.quantity, input.unit ?? null).toFixed(2),
    unit: input.quantity == null ? null : baseUnitOf(dimensionOf(input.unit ?? null)),
    aisle: input.aisle,
    isManual: true,
  })

  return getGroceryListForWeek(db, userId, now)
}

export async function deleteGroceryItem(db: Db, userId: string, itemId: string, now = new Date()): Promise<GroceryListDto | null> {
  if (!(await ownsItem(db, userId, itemId))) return null
  await db.delete(groceryItems).where(eq(groceryItems.id, itemId))
  return getGroceryListForWeek(db, userId, now)
}

/** Fusionne plusieurs articles en un seul (regroupement manuel). Le premier id
    est conservé : libellé et rayon gardés, quantités additionnées si tous les
    articles partagent la même unité de base (sinon quantité effacée — le
    détail reste dans les sources). */
export async function mergeGroceryItems(db: Db, userId: string, itemIds: string[], now = new Date()): Promise<GroceryListDto | null> {
  const rows = await db.select({
    id: groceryItems.id,
    groceryListId: groceryItems.groceryListId,
    quantity: groceryItems.quantity,
    unit: groceryItems.unit,
    checked: groceryItems.checked,
    isManual: groceryItems.isManual,
    sources: groceryItems.sources,
  })
    .from(groceryItems)
    .innerJoin(groceryLists, eq(groceryLists.id, groceryItems.groceryListId))
    .innerJoin(mealPlans, eq(mealPlans.id, groceryLists.mealPlanId))
    .where(and(inArray(groceryItems.id, itemIds), eq(mealPlans.userId, userId)))

  // Tous les articles doivent exister, appartenir à l'utilisateur et à la même liste.
  if (rows.length !== itemIds.length) return null
  if (new Set(rows.map(r => r.groceryListId)).size !== 1) return null

  const byId = new Map(rows.map(r => [r.id, r]))
  const ordered = itemIds.map(id => byId.get(id)!)
  const [target, ...merged] = ordered

  const units = new Set(ordered.map(r => r.unit))
  const summable = units.size === 1 && ordered.every(r => r.quantity !== null)
  const quantity = summable
    ? ordered.reduce((total, r) => total + Number(r.quantity), 0)
    : null

  await db.transaction(async (tx) => {
    await tx.update(groceryItems)
      .set({
        quantity: quantity === null ? null : quantity.toFixed(2),
        unit: summable ? target!.unit : null,
        // Regroupé pour l'achat : repasse « à acheter » même si une part était cochée.
        checked: false,
        isManual: ordered.every(r => r.isManual),
        sources: ordered.flatMap(r => r.sources ?? []),
      })
      .where(eq(groceryItems.id, target!.id))
    await tx.delete(groceryItems).where(inArray(groceryItems.id, merged.map(r => r.id)))
  })

  return getGroceryListForWeek(db, userId, now)
}
