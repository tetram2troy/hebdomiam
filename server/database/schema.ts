import { pgEnum, pgTable, text, timestamp, uuid } from 'drizzle-orm/pg-core'

export const localeEnum = pgEnum('locale', ['fr', 'en'])
export const unitSystemEnum = pgEnum('unit_system', ['metric', 'imperial'])

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
