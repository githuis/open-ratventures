use axum::{Extension, Json, Router, extract::Path, http::StatusCode, routing::{get, post, put}};

use crate::data::{CharacterWrapper, Unit};
use crate::db::DbConnection;

pub fn routes() -> Router {
    Router::new()
        .route("/character", post(create_character))
        .route("/character/{user_id}", get(get_character))
        .route("/character/{user_id}/unit", put(update_unit))
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

async fn update_unit(
    Extension(db): Extension<DbConnection>,
    Path(user_id): Path<i32>,
    Json(unit): Json<Unit>,
) -> StatusCode {
    match db.update_unit_for_user(user_id, &unit).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
