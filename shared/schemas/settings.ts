import { z } from 'zod'

export const profileUpdateSchema = z.object({
  name: z.string().trim().min(1).max(100).optional(),
  locale: z.enum(['fr', 'en']).optional(),
  unitSystem: z.enum(['metric', 'imperial']).optional(),
  defaultDishCount: z.number().int().min(1).max(10).optional(),
  defaultPeopleCount: z.number().int().min(1).max(12).optional(),
  defaultDaysCount: z.number().int().min(1).max(7).optional(),
  rotationWeeks: z.number().int().min(1).max(12).optional(),
}).refine(patch => Object.keys(patch).length > 0, { message: 'empty patch' })

export const passwordChangeSchema = z.object({
  currentPassword: z.string().min(1).max(128),
  newPassword: z.string().min(12).max(128),
})

export const accountDeleteSchema = z.object({
  password: z.string().min(1).max(128),
})

export type ProfileUpdateInput = z.infer<typeof profileUpdateSchema>
