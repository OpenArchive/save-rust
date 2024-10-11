use actix_web::{web, get, post, delete, HttpResponse, Responder, Scope};
use serde::Deserialize;
use serde_json::json;
use futures::StreamExt;
use crate::error::AppResult;
use crate::models::SnowbirdRepo;
use crate::server::server::{get_backend, GroupRepoPath};
use crate::utils::create_veilid_cryptokey_from_base64;
use save_dweb_backend::common::DHTEntity;

pub fn scope() -> Scope {
    web::scope("/groups/{group_id}/repos")
        .service(create_repo)
        .service(get_repo)
        .service(list_repos) 
}

#[derive(Deserialize)]
struct CreateRepoRequest {
    name: String
}

#[derive(Deserialize)]
struct MediaQuery {
    file_name: String,
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

#[get("")]
async fn list_repos(path: web::Path<String>) -> AppResult<impl Responder> {
    let group_id = path.into_inner();

    // Fetch the backend and the group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Call the list_repos method to get the list of repo IDs
    let repo_ids = group.list_repos();

    // Return the list of repos as JSON
    Ok(HttpResponse::Ok().json(repo_ids))
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

#[post("/{repo_id}/media")]
async fn upload_file(
    path: web::Path<GroupRepoPath>,
    query: web::Query<MediaQuery>,  
    mut body: web::Payload,
) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;
    let file_name = &query.file_name;  

    // Fetch the backend and group with proper error handling
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)
        .map_err(|e| anyhow::anyhow!("Invalid group id: {}", e))?;
    let backend = get_backend().await.map_err(|e| anyhow::anyhow!("Failed to get backend: {}", e))?;
    let group = backend.get_group(&crypto_key).await.map_err(|e| anyhow::anyhow!("Failed to get group: {}", e))?;

    // Fetch the repo with proper error handling
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)
        .map_err(|e| anyhow::anyhow!("Invalid repo id: {}", e))?;
    let repo = group.get_repo(&repo_crypto_key).map_err(|e| anyhow::anyhow!("Repo not found: {}", e))?;

    // Log file_name and stream file content
    log::info!("Uploading file: {}", file_name);
    let mut file_data: Vec<u8> = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("Failed to read file chunk: {}", e))?;
        file_data.extend_from_slice(&chunk);
    }

    // Validate file content
    if file_data.is_empty() {
        return Err(anyhow::anyhow!("File content is empty").into());
    }

    log::info!("Uploading file: {}", file_name);

    // Upload the file
    let file_hash = repo.upload(file_name, file_data).await
        .map_err(|e| anyhow::anyhow!("Failed to upload file: {}", e))?;

    log::info!("Updating DHT with hash: {}", file_hash);

    // After uploading, update the DHT with the new file’s hash
    let updated_collection_hash = repo
        .set_file_and_update_dht(&repo.get_name().await?, file_name, &file_hash)
        .await.map_err(|e| anyhow::anyhow!("Failed to update DHT: {}", e))?;

    Ok(HttpResponse::Ok().json(json!({
        "file_hash": file_hash,
        "updated_collection_hash": updated_collection_hash,
    })))
}


#[get("/{repo_id}/list_files")]
async fn list_files(
    path: web::Path<GroupRepoPath>,
) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)?;
    let repo = group.get_repo(&repo_crypto_key)?;

    // List files and check if they are downloaded
    let files = repo.list_files().await?;
    let mut files_with_status = Vec::new();

    for file_name in files {
        let file_hash = match repo.get_file_hash(&file_name).await {
            Ok(hash) => hash,
            Err(_) => continue, // Handle the error or skip the file if there's an issue
        };
        let is_downloaded = repo.has_hash(&file_hash).await.unwrap_or(false); // Check if the file is downloaded
        files_with_status.push(json!({
            "name": file_name,
            "hash": file_hash,
            "is_downloaded": is_downloaded
        }));
    }

    Ok(HttpResponse::Ok().json(files_with_status))
}

#[get("/{repo_id}/media")]
async fn download_file(
    path: web::Path<GroupRepoPath>,
    query: web::Query<MediaQuery>,  
) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;
    let file_name = &query.file_name; 

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)?;
    let repo = group.get_repo(&repo_crypto_key)?;

    // Get the file hash
    let file_hash = repo.get_file_hash(file_name).await?;

    // Trigger file download from peers using the hash
    let file_data = group.download_hash_from_peers(&file_hash).await.map_err(|e| {
        anyhow::anyhow!("Failed to download file from peers: {}", e)
    })?;

    // Return the file data as a binary response
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .body(file_data))
}



#[delete("/{repo_id}/media")]
async fn delete_file(
    path: web::Path<GroupRepoPath>,
    query: web::Query<MediaQuery>,
) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;
    let file_name = &query.file_name;

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)?;
    let repo = group.get_repo(&repo_crypto_key)?;

    // Delete the file and update the collection
    let collection_hash = repo.delete_file(file_name).await?;
    
    Ok(HttpResponse::Ok().json(collection_hash))
}
