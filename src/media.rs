use crate::constants::TAG;
use crate::error::AppResult;
use crate::logging::android_log;
use crate::models::{GroupRepoMediaPath, GroupRepoPath};
use crate::server::server::get_backend;
use crate::utils::create_veilid_cryptokey_from_base64;
use crate::{log_debug, log_info};
use actix_web::web::Bytes;
use actix_web::{delete, get, post, web, HttpResponse, Responder, Scope};
use futures::StreamExt;
use save_dweb_backend::common::DHTEntity;
use serde_json::json;

pub fn scope() -> Scope {
    web::scope("/media")
        .service(upload_file)
        .service(list_files)
        .service(delete_file)
        .service(download_file)
}

#[get("")]
async fn list_files(path: web::Path<GroupRepoPath>) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)?;
    let repo = group.get_repo(&repo_crypto_key).await?;

    let hash = repo.get_hash_from_dht().await?;
    if !group.has_hash(&hash).await? {
        group.download_hash_from_peers(&hash).await?;
    }

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

#[get("/{file_name}")]
async fn download_file(path: web::Path<GroupRepoMediaPath>) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;
    let file_name = &path_params.file_name;

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)?;
    let repo = group.get_repo(&repo_crypto_key).await?;

    if !repo.can_write() {
        let collection_hash = repo.get_hash_from_dht().await?;
        if !group.has_hash(&collection_hash).await? {
            group.download_hash_from_peers(&collection_hash).await?;
        }
    }

    // Get the file hash
    let file_hash = repo.get_file_hash(&file_name).await?;

    if !repo.can_write() {
        if !group.has_hash(&file_hash).await? {
            group.download_hash_from_peers(&file_hash).await?;
        }
    }
    // Trigger file download from peers using the hash
    let file_data = repo
        .get_file_stream(file_name)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to download file from peers: {}", e))?;

    // Return the file data as a binary response
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .streaming(file_data))
}

#[delete("/{file_name}")]
async fn delete_file(path: web::Path<GroupRepoMediaPath>) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;
    let file_name = &path_params.file_name;

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)?;
    let repo = group.get_repo(&repo_crypto_key).await?;

    // Delete the file and update the collection
    let collection_hash = repo.delete_file(&file_name).await?;

    Ok(HttpResponse::Ok().json(collection_hash))
}

#[post("/{file_name}")]
async fn upload_file(
    path: web::Path<GroupRepoMediaPath>,
    mut body: web::Payload,
) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;
    let file_name = &path_params.file_name;

    // Fetch the backend and group with proper error handling
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)
        .map_err(|e| anyhow::anyhow!("Invalid group id: {}", e))?;
    let backend = get_backend()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get backend: {}", e))?;
    let group = backend
        .get_group(&crypto_key)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get group: {}", e))?;

    // Fetch the repo with proper error handling
    let repo_crypto_key = create_veilid_cryptokey_from_base64(&repo_id)
        .map_err(|e| anyhow::anyhow!("Invalid repo id: {}", e))?;
    let repo = group
        .get_repo(&repo_crypto_key)
        .await
        .map_err(|e| anyhow::anyhow!("Repo not found: {}", e))?;

    // Log file_name and stream file content

    log_info!(TAG, "Uploading file: {}", file_name);

    let mut file_data: Vec<u8> = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("Failed to read file chunk: {}", e))?;
        file_data.extend_from_slice(&chunk);
    }

    // Validate file content
    if file_data.is_empty() {
        return Err(anyhow::anyhow!("File content is empty").into());
    }

    // Upload the file
    let updated_collection_hash = repo
        .upload(&file_name, file_data)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to upload file: {}", e))?;

    Ok(HttpResponse::Ok().json(json!({
        "updated_collection_hash": updated_collection_hash,
    })))
}
