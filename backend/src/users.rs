use axum::{Extension, Json, Router, extract::Path, http::StatusCode, routing::{get, post}};

use crate::data::CharacterWrapper;
use crate::db::DbConnection;

pub fn routes() -> Router {
    Router::new()
        .route("/character", post(create_character))
        .route("/character/{user_id}", get(get_character))
}

async fn create_character(
    Extension(db): Extension<DbConnection>,
    user_id: String,
) -> Json<CharacterWrapper> {
    Json(db.get_character(user_id).await.unwrap())
}

async fn get_character(
    Extension(db): Extension<DbConnection>,
    Path(user_id): Path<i32>,
) -> Result<Json<CharacterWrapper>, StatusCode> {
    db.get_character_by_user_id(user_id).await.map(Json).map_err(|_| StatusCode::NOT_FOUND)
}
