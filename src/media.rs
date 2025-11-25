use crate::constants::TAG;
use crate::error::{AppError, AppResult};
use crate::models::{GroupRepoMediaPath, GroupRepoPath};
use crate::server::get_backend;
use crate::utils::create_veilid_cryptokey_from_base64;
use crate::log_info;
use actix_web::{delete, get, post, web, HttpResponse, Responder, Scope, http::header, error::BlockingError};
use bytes::{BytesMut, Bytes};
use futures::Stream;
use futures::StreamExt;
use serde_json::json;
use std::io;

pub fn scope() -> Scope {
    web::scope("/media")
        .service(upload_file)
        .service(list_files)
        .service(delete_file)
        .service(download_file)
}

pub fn from_blocking<T>(result: Result<T, BlockingError>) -> AppResult<T> {
    result.map_err(AppError::from)
}

async fn handle_file_stream(mut file_data: impl Stream<Item = Result<Bytes, io::Error>> + Unpin) -> AppResult<(usize, Bytes)> {
    let mut buffer = BytesMut::new();
    let mut length = 0;

    while let Some(chunk_result) = file_data.next().await {
        let chunk = chunk_result.map_err(|e| AppError(anyhow::Error::new(e)))?;
        buffer.extend_from_slice(&chunk);
        length += chunk.len();
    }

    let final_buffer = web::block(move || {
        buffer.freeze()
    }).await?;

    Ok((length, final_buffer))
}

#[get("")]
async fn list_files(path: web::Path<GroupRepoPath>) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(repo_id)?;
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
    Ok(HttpResponse::Ok().json(json!({ "files": files_with_status })))
}

#[get("/{file_name}")]
async fn download_file(path: web::Path<GroupRepoMediaPath>) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;
    let file_name = &path_params.file_name;

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(repo_id)?;
    let repo = group.get_repo(&repo_crypto_key).await?;

    if !repo.can_write() {
        let collection_hash = repo.get_hash_from_dht().await?;
        if !group.has_hash(&collection_hash).await? {
            group.download_hash_from_peers(&collection_hash).await?;
        }
    }

    // Get the file hash
    let file_hash = repo.get_file_hash(file_name).await?;

    if !repo.can_write() && !group.has_hash(&file_hash).await? {
        group.download_hash_from_peers(&file_hash).await?;
    }
    // Trigger file download from peers using the hash
    let file_data = repo
        .get_file_stream(file_name)
        .await?;

    let (content_length, buffered_data) = handle_file_stream(file_data).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header((header::CONTENT_LENGTH, content_length))
        .body(buffered_data))
}

#[delete("/{file_name}")]
async fn delete_file(path: web::Path<GroupRepoMediaPath>) -> AppResult<impl Responder> {
    let path_params = path.into_inner();
    let group_id = &path_params.group_id;
    let repo_id = &path_params.repo_id;
    let file_name = &path_params.file_name;

    // Fetch the backend and group
    let crypto_key = create_veilid_cryptokey_from_base64(group_id)?;
    let backend = get_backend().await?;
    let group = backend.get_group(&crypto_key).await?;

    // Fetch the repo
    let repo_crypto_key = create_veilid_cryptokey_from_base64(repo_id)?;
    let repo = group.get_repo(&repo_crypto_key).await?;

    // Delete the file and update the collection
    let collection_hash = repo.delete_file(file_name).await?;

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
    let crypto_key = create_veilid_cryptokey_from_base64(group_id)
        .map_err(|e| anyhow::anyhow!("Invalid group id: {e}"))?;
    let backend = get_backend()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get backend: {e}"))?;
    let group = backend
        .get_group(&crypto_key)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get group: {e}"))?;

    // Fetch the repo with proper error handling
    let repo_crypto_key = create_veilid_cryptokey_from_base64(repo_id)
        .map_err(|e| anyhow::anyhow!("Invalid repo id: {e}"))?;
    let repo = group
        .get_repo(&repo_crypto_key)
        .await
        .map_err(|e| anyhow::anyhow!("Repo not found: {e}"))?;

    // Log file_name and stream file content

    log_info!(TAG, "Uploading file: {}", file_name);

    let mut file_data: Vec<u8> = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("Failed to read file chunk: {e}"))?;
        file_data.extend_from_slice(&chunk);
    }

    // Validate file content
    if file_data.is_empty() {
        return Err(anyhow::anyhow!("File content is empty").into());
    }

    // Upload the file
    let updated_collection_hash = repo
        .upload(file_name, file_data)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to upload file: {e}"))?;

    Ok(HttpResponse::Ok().json(json!({
        "name": file_name,
        "updated_collection_hash": updated_collection_hash,
    })))
}
