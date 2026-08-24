import { closeDb } from '../utils/db'
import { logError } from '../utils/logger'

// Arrêt propre (docs/HOSTING.md §2.7) : Nitro draine les requêtes sur SIGTERM
// puis déclenche le hook `close` — on y ferme le pool PostgreSQL, sinon les
// connexions ouvertes retiennent le processus jusqu'au timeout (puis SIGKILL).
export default defineNitroPlugin((nitroApp) => {
  nitroApp.hooks.hook('close', async () => {
    try {
      await closeDb()
    }
    catch (error) {
      logError('db.close_failed', error)
    }
  })
})
