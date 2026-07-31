import { z } from 'zod'
import { emailSchema } from './auth'

// Entrées des routes /api/admin/* — listes blanches strictes : aucun champ
// non déclaré ici ne peut atteindre la base (pas de passwordHash, pas de rôle
// à la création : tout compte invité naît 'user').

export const adminCreateUserSchema = z.object({
  email: emailSchema,
  name: z.string().trim().min(1).max(100),
})

export const adminUpdateUserSchema = z.object({
  role: z.enum(['admin', 'user']).optional(),
  disabled: z.boolean().optional(),
}).refine(patch => Object.keys(patch).length > 0, { message: 'empty patch' })

export const adminUserIdParamsSchema = z.object({
  id: z.uuid(),
})

export type AdminCreateUserInput = z.infer<typeof adminCreateUserSchema>
export type AdminUpdateUserInput = z.infer<typeof adminUpdateUserSchema>
