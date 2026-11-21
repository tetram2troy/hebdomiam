-- Promotion rétroactive : « premier inscrit = admin » s'applique aussi aux
-- instances existantes — le plus ancien compte devient administrateur.
-- No-op sûr sur une base vierge (sous-requête vide). Migration séparée de 0005
-- pour garder schéma et données distincts (et rejouables indépendamment).
UPDATE "users" SET "role" = 'admin'
WHERE "id" = (SELECT "id" FROM "users" ORDER BY "created_at" ASC LIMIT 1);
