import { z } from 'zod'

export const emailSchema = z.string().trim().toLowerCase().max(255).pipe(z.email())

export const registerSchema = z.object({
  email: emailSchema,
  password: z.string().min(12).max(128),
  name: z.string().trim().min(1).max(100),
})

export const loginSchema = z.object({
  email: emailSchema,
  password: z.string().min(1).max(128),
})

export const totpCodeSchema = z.object({
  code: z.string().trim().min(6).max(16),
})

export const forgotPasswordSchema = z.object({
  email: emailSchema,
})

export const resetPasswordSchema = z.object({
  token: z.string().min(32).max(128),
  password: z.string().min(12).max(128),
})

export const verifyEmailSchema = z.object({
  token: z.string().min(32).max(128),
})

/** Acceptation d'une invitation créée par un admin : le lien prouve la
    possession du courriel, l'utilisateur choisit son mot de passe. */
export const acceptInviteSchema = z.object({
  token: z.string().min(32).max(128),
  password: z.string().min(12).max(128),
})

export type RegisterInput = z.infer<typeof registerSchema>
export type LoginInput = z.infer<typeof loginSchema>
