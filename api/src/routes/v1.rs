//! Routes ouvertes aux clés d'API (`Authorization: Bearer hbd_…`) — portée
//! minimale : enregistrer une recette et lire les tags existants (pour les
//! réutiliser plutôt qu'en créer des quasi-doublons). Rien d'autre.
//!
//! Contrairement à l'import par URL (F3, brouillon relu avant sauvegarde), la
//! recette est enregistrée directement : c'est l'utilisateur qui pilote l'outil
//! appelant (Claude Code), en voit le compte rendu et corrige ensuite dans
//! l'app — exception documentée au PRD §12.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use super::recipes::{RecipeBody, create_recipe};
use crate::api_key::ApiKeyUser;
use crate::error::ApiResult;
use crate::repo::recipes::{self, TagSummary};
use crate::state::AppState;
use crate::validation::ValidJson;

/// Créations par compte et par heure : large pour un usage manuel, bloque une
/// boucle emballée d'un agent.
const CREATE_MAX_PER_HOUR: usize = 30;

pub async fn create_recipe_with_key(
    State(state): State<AppState>,
    auth: ApiKeyUser,
    ValidJson(input): ValidJson<RecipeBody>,
) -> ApiResult<impl IntoResponse> {
    state.rate_limit(
        "v1:recipes:create",
        &auth.user_id.to_string(),
        CREATE_MAX_PER_HOUR,
        60 * 60_000,
    )?;
    let recipe = create_recipe(&state, auth.user_id, &input, Some(auth.key_id)).await?;
    Ok((StatusCode::CREATED, Json(recipe)))
}

pub async fn tags(State(state): State<AppState>, auth: ApiKeyUser) -> ApiResult<Json<Vec<TagSummary>>> {
    Ok(Json(recipes::list_tags(&state.db, auth.user_id).await?))
}
