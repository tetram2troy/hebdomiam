import { relations } from 'drizzle-orm'
import { boolean, date, index, integer, jsonb, numeric, pgEnum, pgTable, primaryKey, text, timestamp, uniqueIndex, uuid } from 'drizzle-orm/pg-core'

export const localeEnum = pgEnum('locale', ['fr', 'en'])
export const unitSystemEnum = pgEnum('unit_system', ['metric', 'imperial'])
export const mealPlanStatusEnum = pgEnum('meal_plan_status', ['draft', 'validated', 'cooked'])
export const mealPlanRecipeStatusEnum = pgEnum('meal_plan_recipe_status', ['suggested', 'accepted', 'swapped', 'cooked'])
export const emailTokenTypeEnum = pgEnum('email_token_type', ['verify', 'reset', 'invite'])
export const userRoleEnum = pgEnum('user_role', ['admin', 'user'])

// Enums partagés avec shared/schemas/recipe.ts (UNITS / AISLES) — garder synchronisés.
export const unitEnum = pgEnum('unit', ['g', 'kg', 'ml', 'l', 'oz', 'lb', 'cup', 'tbsp', 'tsp', 'unit', 'pinch'])
export const aisleEnum = pgEnum('aisle', ['produce', 'butcher', 'fish', 'dairy', 'dry', 'canned', 'frozen', 'bakery', 'spices', 'beverages', 'household', 'other'])

export const users = pgTable('users', {
  id: uuid('id').primaryKey().defaultRandom(),
  email: text('email').notNull().unique(),
  passwordHash: text('password_hash').notNull(),
  name: text('name').notNull(),
  emailVerifiedAt: timestamp('email_verified_at', { withTimezone: true }),
  // Rôles : l'inscription publique est fermée — les comptes sont créés par un admin
  // (le premier inscrit d'une instance vierge est promu admin, voir users-repo).
  role: userRoleEnum('role').notNull().default('user'),
  // Désactivation douce : bloque la connexion et invalide les sessions en cours
  // (middleware account-status) sans supprimer les données.
  disabledAt: timestamp('disabled_at', { withTimezone: true }),
  locale: localeEnum('locale').notNull().default('fr'),
  unitSystem: unitSystemEnum('unit_system').notNull().default('metric'),
  // Valeurs par défaut de planification (modifiables dans les réglages, PRD F5)
  defaultDishCount: integer('default_dish_count').notNull().default(4),
  defaultPeopleCount: integer('default_people_count').notNull().default(1),
  defaultDaysCount: integer('default_days_count').notNull().default(7),
  rotationWeeks: integer('rotation_weeks').notNull().default(3),
  // 2FA TOTP optionnelle (M5) : secret posé au setup, actif quand enabledAt non nul.
  totpSecret: text('totp_secret'),
  totpEnabledAt: timestamp('totp_enabled_at', { withTimezone: true }),
  totpBackupHash: text('totp_backup_hash'),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
})

/** Jetons de courriel (vérification, réinitialisation) — hachés, à expiration. */
export const emailTokens = pgTable('email_tokens', {
  id: uuid('id').primaryKey().defaultRandom(),
  userId: uuid('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),
  type: emailTokenTypeEnum('type').notNull(),
  tokenHash: text('token_hash').notNull().unique(),
  expiresAt: timestamp('expires_at', { withTimezone: true }).notNull(),
  usedAt: timestamp('used_at', { withTimezone: true }),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
}, table => [
  index('email_tokens_user_id_idx').on(table.userId),
])

export const recipes = pgTable('recipes', {
  id: uuid('id').primaryKey().defaultRandom(),
  userId: uuid('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),
  title: text('title').notNull(),
  description: text('description'),
  imageUrl: text('image_url'),
  baseServings: integer('base_servings').notNull(),
  prepMinutes: integer('prep_minutes'),
  cookMinutes: integer('cook_minutes'),
  sourceUrl: text('source_url'),
  freezable: boolean('freezable').notNull().default(false),
  notes: text('notes'),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
  updatedAt: timestamp('updated_at', { withTimezone: true }).notNull().defaultNow(),
  // Suppression douce : les requêtes métier filtrent toujours deletedAt IS NULL.
  deletedAt: timestamp('deleted_at', { withTimezone: true }),
}, table => [
  index('recipes_user_id_idx').on(table.userId),
])

export const recipeSteps = pgTable('recipe_steps', {
  id: uuid('id').primaryKey().defaultRandom(),
  recipeId: uuid('recipe_id').notNull().references(() => recipes.id, { onDelete: 'cascade' }),
  position: integer('position').notNull(),
  text: text('text').notNull(),
}, table => [
  index('recipe_steps_recipe_id_idx').on(table.recipeId),
])

export const recipeIngredients = pgTable('recipe_ingredients', {
  id: uuid('id').primaryKey().defaultRandom(),
  recipeId: uuid('recipe_id').notNull().references(() => recipes.id, { onDelete: 'cascade' }),
  position: integer('position').notNull(),
  // decimal, jamais float (PRD §6) — mise à l'échelle et agrégation exactes.
  quantity: numeric('quantity', { precision: 10, scale: 2 }).notNull(),
  unit: unitEnum('unit'),
  name: text('name').notNull(),
  aisle: aisleEnum('aisle').notNull().default('other'),
}, table => [
  index('recipe_ingredients_recipe_id_idx').on(table.recipeId),
])

export const tags = pgTable('tags', {
  id: uuid('id').primaryKey().defaultRandom(),
  userId: uuid('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),
  name: text('name').notNull(),
}, table => [
  uniqueIndex('tags_user_id_name_idx').on(table.userId, table.name),
])

export const recipeTags = pgTable('recipe_tags', {
  recipeId: uuid('recipe_id').notNull().references(() => recipes.id, { onDelete: 'cascade' }),
  tagId: uuid('tag_id').notNull().references(() => tags.id, { onDelete: 'cascade' }),
}, table => [
  primaryKey({ columns: [table.recipeId, table.tagId] }),
])

export const mealPlans = pgTable('meal_plans', {
  id: uuid('id').primaryKey().defaultRandom(),
  userId: uuid('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),
  weekStartDate: date('week_start_date').notNull(),
  peopleCount: integer('people_count').notNull(),
  dishCount: integer('dish_count').notNull(),
  daysCount: integer('days_count').notNull(),
  totalServings: integer('total_servings').notNull(),
  status: mealPlanStatusEnum('status').notNull().default('draft'),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
}, table => [
  // Un seul plan par semaine et par utilisateur (« Replanifier » remplace).
  uniqueIndex('meal_plans_user_id_week_idx').on(table.userId, table.weekStartDate),
])

export const mealPlanRecipes = pgTable('meal_plan_recipes', {
  id: uuid('id').primaryKey().defaultRandom(),
  mealPlanId: uuid('meal_plan_id').notNull().references(() => mealPlans.id, { onDelete: 'cascade' }),
  recipeId: uuid('recipe_id').notNull().references(() => recipes.id, { onDelete: 'cascade' }),
  position: integer('position').notNull(),
  targetServings: integer('target_servings').notNull(),
  // « swapped » reste en base : mémoire des suggestions écartées pour ne pas les
  // reproposer ; « cooked » alimente la rotation (historique).
  status: mealPlanRecipeStatusEnum('status').notNull().default('suggested'),
  cookedAt: timestamp('cooked_at', { withTimezone: true }),
}, table => [
  index('meal_plan_recipes_meal_plan_id_idx').on(table.mealPlanId),
  index('meal_plan_recipes_recipe_id_idx').on(table.recipeId),
])

export const groceryLists = pgTable('grocery_lists', {
  id: uuid('id').primaryKey().defaultRandom(),
  mealPlanId: uuid('meal_plan_id').notNull().unique().references(() => mealPlans.id, { onDelete: 'cascade' }),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
})

/** Contribution d'un ingrédient de recette à un article agrégé (instantané figé
    à la génération — quantités déjà mises à l'échelle, unité d'origine). */
export interface GroceryItemSource {
  ingredientId: string
  recipeTitle: string
  quantity: number
  unit: string | null
}

export const groceryItems = pgTable('grocery_items', {
  id: uuid('id').primaryKey().defaultRandom(),
  groceryListId: uuid('grocery_list_id').notNull().references(() => groceryLists.id, { onDelete: 'cascade' }),
  label: text('label').notNull(),
  // Quantité stockée en unité de BASE (g pour la masse, ml pour le volume) ;
  // l'affichage convertit selon la préférence utilisateur (unitSystem, D6).
  quantity: numeric('quantity', { precision: 12, scale: 2 }),
  unit: unitEnum('unit'),
  aisle: aisleEnum('aisle').notNull().default('other'),
  checked: boolean('checked').notNull().default(false),
  isManual: boolean('is_manual').notNull().default(false),
  sources: jsonb('sources').$type<GroceryItemSource[]>(),
}, table => [
  index('grocery_items_grocery_list_id_idx').on(table.groceryListId),
])

export const recipesRelations = relations(recipes, ({ many }) => ({
  steps: many(recipeSteps),
  ingredients: many(recipeIngredients),
  recipeTags: many(recipeTags),
}))

export const groceryListsRelations = relations(groceryLists, ({ one, many }) => ({
  mealPlan: one(mealPlans, { fields: [groceryLists.mealPlanId], references: [mealPlans.id] }),
  items: many(groceryItems),
}))

export const groceryItemsRelations = relations(groceryItems, ({ one }) => ({
  groceryList: one(groceryLists, { fields: [groceryItems.groceryListId], references: [groceryLists.id] }),
}))

export const mealPlansRelations = relations(mealPlans, ({ many }) => ({
  planRecipes: many(mealPlanRecipes),
}))

export const mealPlanRecipesRelations = relations(mealPlanRecipes, ({ one }) => ({
  mealPlan: one(mealPlans, { fields: [mealPlanRecipes.mealPlanId], references: [mealPlans.id] }),
  recipe: one(recipes, { fields: [mealPlanRecipes.recipeId], references: [recipes.id] }),
}))

export const recipeStepsRelations = relations(recipeSteps, ({ one }) => ({
  recipe: one(recipes, { fields: [recipeSteps.recipeId], references: [recipes.id] }),
}))

export const recipeIngredientsRelations = relations(recipeIngredients, ({ one }) => ({
  recipe: one(recipes, { fields: [recipeIngredients.recipeId], references: [recipes.id] }),
}))

export const tagsRelations = relations(tags, ({ many }) => ({
  recipeTags: many(recipeTags),
}))

export const recipeTagsRelations = relations(recipeTags, ({ one }) => ({
  recipe: one(recipes, { fields: [recipeTags.recipeId], references: [recipes.id] }),
  tag: one(tags, { fields: [recipeTags.tagId], references: [tags.id] }),
}))
