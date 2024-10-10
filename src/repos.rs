use actix_web::{web, get, post, HttpResponse, Responder, Scope};
use serde::Deserialize;
use crate::error::AppResult;
use crate::models::SnowbirdRepo;
use crate::server::server::{get_backend, GroupRepoPath};
use crate::utils::create_veilid_cryptokey_from_base64;
use save_dweb_backend::common::DHTEntity;

pub fn scope() -> Scope {
    web::scope("/groups/{group_id}/repos")
        .service(create_repo)
        .service(get_repo)
}

#[derive(Deserialize)]
struct CreateRepoRequest {
    name: String
}

#[get("/{repo_id}")]
async fn get_repo(path: web::Path<GroupRepoPath>) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let _group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;

    // Fetch the backend and the group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    Ok(HttpResponse::Ok().json(snowbird_repo))
}

#[post("")]
async fn create_repo(
    path: web::Path<String>,
    body: web::Json<CreateRepoRequest>
) -> AppResult<impl Responder> {

    let group_id = path.into_inner();
    let repo_data = body.into_inner();

    let mut backend = get_backend().await?;

    let typed_key = create_veilid_typedkey_from_base64(&group_id)?;
    let mut group = backend.get_group(typed_key).await?;

    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let repo = backend.create_repo(&crypto_key).await?;
    repo.set_name(&repo_data.name).await?;

    group.add_repo(repo.clone()).expect("Unable to add repo1");

    let snowbird_repo: SnowbirdRepo = repo.into();
    
    Ok(HttpResponse::Ok().json(snowbird_repo))
}