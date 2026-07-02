import { defineConfig } from 'drizzle-kit'

export default defineConfig({
  dialect: 'postgresql',
  schema: './server/database/schema.ts',
  out: './server/database/migrations',
  dbCredentials: {
    // db:generate fonctionne sans URL ; db:migrate en exige une.
    url: process.env.NUXT_DATABASE_URL ?? '',
  },
})
