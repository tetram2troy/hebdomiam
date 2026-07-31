// Force le traçage du migrator drizzle vers .output/server/node_modules :
// le job de déploiement (node .output/server/migrate.mjs) l'importe, mais
// aucun code de route ne le référence — sans cet import, Nitro ne copierait
// pas le module dans l'image de production. (traceInclude de nitropack 2.x
// ajoute l'identifiant brut au trace et casse le build Linux — vécu.)
// Coût : quelques Ko chargés au boot, jamais exécutés.
import 'drizzle-orm/node-postgres/migrator'

export default defineNitroPlugin(() => {})
