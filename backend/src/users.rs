use axum::{Extension, Json, Router, extract::Path, http::StatusCode, routing::{delete, get, post, put}};
use serde::Deserialize;

#[derive(Deserialize)]
struct CharacterStats { coins: u32, renown: u32 }

use crate::data::{CharacterWrapper, InventoryItem, ShopItem, Unit};
use crate::db::DbConnection;

pub fn routes() -> Router {
    Router::new()
        .route("/character", post(create_character))
        .route("/character/{user_id}", get(get_character))
        .route("/character/{user_id}/unit", put(update_unit))
        .route("/character/{user_id}/stats", put(save_stats))
        .route("/character/{user_id}/items", get(get_items).post(give_item))
        .route("/character/{user_id}/items/{item_id}", delete(consume_item))
        .route("/shop", get(list_shop))
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

async fn give_item(
    Extension(db): Extension<DbConnection>,
    Path(user_id): Path<i32>,
    Json(item_name): Json<String>,
) -> StatusCode {
    match db.give_item_to_character(user_id, &item_name).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
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

async fn get_items(
    Extension(db): Extension<DbConnection>,
    Path(user_id): Path<i32>,
) -> Result<Json<Vec<InventoryItem>>, StatusCode> {
    db.get_character_items(user_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn list_shop(
    Extension(db): Extension<DbConnection>,
) -> Result<Json<Vec<ShopItem>>, StatusCode> {
    db.list_shop_items().await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn save_stats(
    Extension(db): Extension<DbConnection>,
    Path(user_id): Path<i32>,
    Json(stats): Json<CharacterStats>,
) -> StatusCode {
    match db.save_character_stats(user_id, stats.coins, stats.renown).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn consume_item(
    Extension(db): Extension<DbConnection>,
    Path((user_id, item_id)): Path<(i32, i32)>,
) -> StatusCode {
    match db.consume_item(user_id, item_id).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
