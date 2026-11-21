// Énumérations partagées avec l'API Rust (api/src/domain/catalog.rs, types
// enum PostgreSQL) et les clés i18n units.* / aisles.*. Garder synchronisés.
export const UNITS = ['g', 'kg', 'ml', 'l', 'oz', 'lb', 'cup', 'tbsp', 'tsp', 'unit', 'pinch'] as const
export const AISLES = ['produce', 'butcher', 'fish', 'dairy', 'dry', 'canned', 'frozen', 'bakery', 'spices', 'beverages', 'household', 'other'] as const

export type Unit = (typeof UNITS)[number]
export type Aisle = (typeof AISLES)[number]
