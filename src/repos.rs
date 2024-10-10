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
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;

    // Fetch the backend and the group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo from the group
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)?;
    let repo = group.get_repo(&repo_crypto_key);
    
    // Convert the repo into the desired format and return the response
    
    // First, handle the Result to get &Box<Repo>
    let repo_box_ref = repo?;

    // Then, dereference to get &Repo
    let repo_ref = &**repo_box_ref;

    // If Repo implements Clone, clone it to get an owned Repo
    let repo_owned = repo_ref.clone();

    // Now, convert the owned Repo into SnowbirdRepo
    let snowbird_repo: SnowbirdRepo = repo_owned.into();

    Ok(HttpResponse::Ok().json(snowbird_repo))
}

#[post("")]
async fn create_repo(
    path: web::Path<String>,
    body: web::Json<CreateRepoRequest>
) -> AppResult<impl Responder> {

    let group_id = path.into_inner();
    let repo_data = body.into_inner();

    let backend = get_backend().await?;
    
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let mut group = backend.get_group(&crypto_key).await?;

    let repo = group.create_repo().await?;

    repo.set_name(&repo_data.name).await?;

    // First, handle the Result to get &Box<Repo>
    let repo_box_ref = repo;

    // Then, dereference to get &Repo
    let repo_ref = &**repo_box_ref;

    // If Repo implements Clone, clone it to get an owned Repo
    let repo_owned = repo_ref.clone();

    // Now, convert the owned Repo into SnowbirdRepo
    let snowbird_repo: SnowbirdRepo = repo_owned.into();
    
    Ok(HttpResponse::Ok().json(snowbird_repo))
}