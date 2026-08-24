// Alias de /api/health au chemin conventionnel exigé par le contrat
// d'hébergement (docs/HOSTING.md §1.C et §8) — même sonde, même réponse.
export { default } from '../api/health.get'
