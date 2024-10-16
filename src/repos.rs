use crate::constants::TAG;
use crate::error::{AppError, AppResult};
use crate::log_debug;
use crate::media;
use crate::models::{AsyncFrom, GroupPath, GroupRepoPath, SnowbirdRepo};
use crate::server::server::get_backend;
use crate::utils::create_veilid_cryptokey_from_base64;
use actix_web::{get, post, web, HttpResponse, Responder, Scope};
use save_dweb_backend::common::DHTEntity;
use save_dweb_backend::group::Group;
// use save_dweb_backend::repo::Repo;
use serde::Deserialize;
use serde_json::json;

pub fn scope() -> Scope {
    web::scope("/repos")
        .service(create_repo)
        .service(list_repos)
        .service(
            web::scope("/{repo_id}")
                .service(get_repo)
                .service(media::scope()),
        )
}

#[derive(Deserialize)]
struct CreateRepoRequest {
    name: String,
}

#[get("")]
async fn list_repos(path: web::Path<GroupPath>) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    log_debug!(TAG, "group_id = {}", group_id);

    // Fetch the backend and the group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;
    log_debug!(TAG, "got group");

    let snowbird_repos = get_snowbird_repos(&group).await?;
    log_debug!(TAG, "got snowbird repos");

    Ok(HttpResponse::Ok().json(json!({ "repos": snowbird_repos })))
}

#[get("")]
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
    let repo = group.get_repo(&repo_crypto_key).await?;

    // Now, convert the owned Repo into SnowbirdRepo
    let snowbird_repo: SnowbirdRepo = repo.into();

    Ok(HttpResponse::Ok().json(snowbird_repo))
}

#[post("")]
async fn create_repo(
    path: web::Path<String>,
    body: web::Json<CreateRepoRequest>,
) -> AppResult<impl Responder> {
    log_debug!(TAG, "start");

    let group_id = path.into_inner();
    let repo_data = body.into_inner();

    let backend = get_backend().await?;

    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let mut group = backend.get_group(&crypto_key).await?;

    let repo = group.create_repo().await?;

    log_debug!(
        TAG,
        "Setting name '{}' on new repo {}",
        repo_data.name,
        repo.id()
    );
    repo.set_name(&repo_data.name).await?;

    // Now, convert the owned Repo into SnowbirdRepo
    let mut snowbird_repo: SnowbirdRepo = repo.into();

    snowbird_repo.name = repo_data.name.clone();

    log_debug!(TAG, "returning snowbird repo");

    Ok(HttpResponse::Ok().json(snowbird_repo))
}

async fn get_snowbird_repos(group: &Group) -> Result<Vec<SnowbirdRepo>, AppError> {
    log_debug!(TAG, "start");

    let repo_ids = group.list_repos().await;
    let mut snowbird_repos = Vec::new();

    for id in repo_ids {
        log_debug!(TAG, "Repo ID {}", id);
        let repo = group.get_repo(&id).await?;
        let snowbird_repo = SnowbirdRepo::async_from(repo).await;
        snowbird_repos.push(snowbird_repo);
    }

    Ok(snowbird_repos)
}
