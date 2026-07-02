import { z } from 'zod'

// Garder synchronisés avec unitEnum / aisleEnum de server/database/schema.ts
// et les clés i18n units.* / aisles.*.
export const UNITS = ['g', 'kg', 'ml', 'l', 'oz', 'lb', 'cup', 'tbsp', 'tsp', 'unit', 'pinch'] as const
export const AISLES = ['produce', 'butcher', 'fish', 'dairy', 'dry', 'canned', 'frozen', 'bakery', 'spices', 'beverages', 'household', 'other'] as const

export type Unit = (typeof UNITS)[number]
export type Aisle = (typeof AISLES)[number]

export const ingredientInputSchema = z.object({
  quantity: z.number().positive().max(99999),
  unit: z.enum(UNITS).nullable().default(null),
  name: z.string().trim().min(1).max(200),
  aisle: z.enum(AISLES).default('other'),
})

export const recipeInputSchema = z.object({
  title: z.string().trim().min(1).max(200),
  description: z.string().trim().max(2000).nullish(),
  baseServings: z.number().int().min(1).max(50),
  prepMinutes: z.number().int().min(0).max(1440).nullish(),
  cookMinutes: z.number().int().min(0).max(1440).nullish(),
  sourceUrl: z.url().max(500).nullish(),
  freezable: z.boolean().default(false),
  notes: z.string().trim().max(5000).nullish(),
  ingredients: z.array(ingredientInputSchema).min(1).max(100),
  steps: z.array(z.string().trim().min(1).max(2000)).min(1).max(50),
  tags: z.array(z.string().trim().min(1).max(30)).max(20).default([])
    .transform(names => [...new Set(names)]),
})

export const recipeListQuerySchema = z.object({
  q: z.string().trim().min(1).max(100).optional(),
  tag: z.string().trim().min(1).max(30).optional(),
  maxMinutes: z.coerce.number().int().min(1).max(1440).optional(),
})

export const recipeIdParamsSchema = z.object({
  id: z.uuid(),
})

export type IngredientInput = z.infer<typeof ingredientInputSchema>
export type RecipeInput = z.infer<typeof recipeInputSchema>
export type RecipeListQuery = z.infer<typeof recipeListQuerySchema>
