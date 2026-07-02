import { relations } from 'drizzle-orm'
import { boolean, index, integer, numeric, pgEnum, pgTable, primaryKey, text, timestamp, uniqueIndex, uuid } from 'drizzle-orm/pg-core'

export const localeEnum = pgEnum('locale', ['fr', 'en'])
export const unitSystemEnum = pgEnum('unit_system', ['metric', 'imperial'])

// Enums partagés avec shared/schemas/recipe.ts (UNITS / AISLES) — garder synchronisés.
export const unitEnum = pgEnum('unit', ['g', 'kg', 'ml', 'l', 'oz', 'lb', 'cup', 'tbsp', 'tsp', 'unit', 'pinch'])
export const aisleEnum = pgEnum('aisle', ['produce', 'butcher', 'fish', 'dairy', 'dry', 'canned', 'frozen', 'bakery', 'spices', 'beverages', 'household', 'other'])

export const users = pgTable('users', {
  id: uuid('id').primaryKey().defaultRandom(),
  email: text('email').notNull().unique(),
  passwordHash: text('password_hash').notNull(),
  name: text('name').notNull(),
  emailVerifiedAt: timestamp('email_verified_at', { withTimezone: true }),
  locale: localeEnum('locale').notNull().default('fr'),
  unitSystem: unitSystemEnum('unit_system').notNull().default('metric'),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
})

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

export const recipesRelations = relations(recipes, ({ many }) => ({
  steps: many(recipeSteps),
  ingredients: many(recipeIngredients),
  recipeTags: many(recipeTags),
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
