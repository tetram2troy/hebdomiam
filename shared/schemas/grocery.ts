import { z } from 'zod'
import { AISLES, UNITS } from './recipe'

export const groceryItemCreateSchema = z.object({
  label: z.string().trim().min(1).max(120),
  quantity: z.number().positive().max(99999).nullish(),
  unit: z.enum(UNITS).nullish(),
  aisle: z.enum(AISLES).default('other'),
})

export const groceryItemUpdateSchema = z.object({
  checked: z.boolean().optional(),
  aisle: z.enum(AISLES).optional(),
  label: z.string().trim().min(1).max(120).optional(),
  quantity: z.number().positive().max(99999).nullable().optional(),
  unit: z.enum(UNITS).nullable().optional(),
}).refine(patch => Object.keys(patch).length > 0, { message: 'empty patch' })

export const groceryItemIdParamsSchema = z.object({
  id: z.uuid(),
})

export type GroceryItemCreateInput = z.infer<typeof groceryItemCreateSchema>
export type GroceryItemUpdateInput = z.infer<typeof groceryItemUpdateSchema>
