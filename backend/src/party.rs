use axum::{
    Extension, Json, Router,
    extract::Path,
    http::StatusCode,
    routing::{delete, get, post},
};

use crate::data::CharacterWrapper;
use crate::db::DbConnection;
use crate::quest_data::{JoinPartyRequest, Party, PartySummary};

pub fn routes() -> Router {
    Router::new()
        .route("/party", post(create_party))
        .route("/party/open", get(open_parties))
        .route("/party/join", post(join_party))
        .route("/party/leave", delete(leave_party))
        .route("/party/active/{user_id}", get(get_party_for_user))
        .route("/party/{id}/members", get(party_members))
}

async fn create_party(
    Extension(db): Extension<DbConnection>,
    Json(user_id): Json<i32>,
) -> Result<Json<Party>, StatusCode> {
    db.create_party(user_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn open_parties(
    Extension(db): Extension<DbConnection>,
) -> Result<Json<Vec<PartySummary>>, StatusCode> {
    db.list_open_parties()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn join_party(
    Extension(db): Extension<DbConnection>,
    Json(req): Json<JoinPartyRequest>,
) -> Result<Json<Party>, StatusCode> {
    db.join_party(req.party_id, req.user_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn leave_party(
    Extension(db): Extension<DbConnection>,
    Json(user_id): Json<i32>,
) -> Result<StatusCode, StatusCode> {
    db.leave_party(user_id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_party_for_user(
    Extension(db): Extension<DbConnection>,
    Path(user_id): Path<i32>,
) -> Result<Json<Party>, StatusCode> {
    db.get_party_for_user(user_id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn party_members(
    Extension(db): Extension<DbConnection>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<CharacterWrapper>>, StatusCode> {
    db.get_party_members(id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
