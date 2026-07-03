import { z } from 'zod'

export const importUrlSchema = z.object({
  url: z.url().max(2000),
})

export type ImportUrlInput = z.infer<typeof importUrlSchema>
