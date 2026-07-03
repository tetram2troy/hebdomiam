import { z } from 'zod'

export const planCreateSchema = z.object({
  dishCount: z.number().int().min(1).max(10),
  peopleCount: z.number().int().min(1).max(12),
  daysCount: z.number().int().min(1).max(7),
  /** Remplace le plan existant de la semaine (« Replanifier »). */
  replace: z.boolean().default(false),
})

export const planIdParamsSchema = z.object({
  id: z.uuid(),
})

export const planRecipeParamsSchema = z.object({
  id: z.uuid(),
  itemId: z.uuid(),
})

export const planRecipeActionSchema = z.object({
  action: z.enum(['accept', 'unaccept', 'cook']),
})

export const planReplaceBodySchema = z.object({
  recipeId: z.uuid(),
})

export const replacementQuerySchema = z.object({
  mode: z.enum(['search', 'suggest']).default('suggest'),
  q: z.string().trim().min(1).max(100).optional(),
})

export type PlanCreateInput = z.infer<typeof planCreateSchema>
export type PlanRecipeAction = z.infer<typeof planRecipeActionSchema>['action']
export type ReplacementQuery = z.infer<typeof replacementQuerySchema>
