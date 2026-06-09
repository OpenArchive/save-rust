use crate::constants::TAG;
use crate::error::{AppError, AppResult};
use crate::log_info;
use crate::models::{GroupRepoMediaPath, GroupRepoPath};
use crate::server::get_backend;
use crate::utils::create_veilid_cryptokey_from_base64;
use actix_web::{
    delete, error::BlockingError, get, http::header, post, web, HttpResponse, Responder, Scope,
};
use bytes::{Bytes, BytesMut};
use futures::Stream;
use futures::StreamExt;
use serde_json::json;
use std::io;
use std::time::{Duration, Instant};

const MEDIA_DOWNLOAD_MAX_ATTEMPTS: u32 = 3;
const MEDIA_DOWNLOAD_PER_PEER_TIMEOUT: Duration = Duration::from_secs(18);
const MEDIA_DOWNLOAD_OVERALL_TIMEOUT: Duration = Duration::from_secs(55);
const MEDIA_DOWNLOAD_INITIAL_BACKOFF: Duration = Duration::from_millis(500);

// TODO: fold this media-specific overall budget into save-dweb-backend's
// Group::download_hash_from_peers once the backend crate exposes that knob.
async fn download_hash_for_media(
    group: &save_dweb_backend::group::Group,
    hash: &iroh_blobs::Hash,
) -> AppResult<()> {
    let mut last_error = None;
    let started = Instant::now();

    for attempt in 1..=MEDIA_DOWNLOAD_MAX_ATTEMPTS {
        let mut peer_repos = group.list_peer_repos().await;
        if peer_repos.is_empty() {
            return Err(anyhow::anyhow!("Cannot download hash. No other peers found").into());
        }

        let peer_count = peer_repos.len();
        peer_repos.rotate_left((attempt as usize - 1) % peer_count);

        for peer_repo in peer_repos {
            let peer_id = peer_repo.id().to_string();

            let Some(remaining) = MEDIA_DOWNLOAD_OVERALL_TIMEOUT.checked_sub(started.elapsed())
            else {
                let detail = format!(
                    "Timed out downloading hash {hash} after overall {}s media budget",
                    MEDIA_DOWNLOAD_OVERALL_TIMEOUT.as_secs()
                );
                log_info!(TAG, "{}", detail);
                let last_detail = match last_error {
                    Some(error) => format!("{detail}; last peer error: {error}"),
                    None => detail,
                };
                return Err(anyhow::anyhow!(
                    "Unable to download hash {} from any peer after {} media attempts; last error: {}",
                    hash,
                    attempt.saturating_sub(1),
                    last_detail
                )
                .into());
            };

            log_info!(
                TAG,
                "Media download attempt {}/{} for hash {} from peer {}",
                attempt,
                MEDIA_DOWNLOAD_MAX_ATTEMPTS,
                hash,
                peer_id
            );

            let timeout_budget = std::cmp::min(MEDIA_DOWNLOAD_PER_PEER_TIMEOUT, remaining);
            let download_result = tokio::time::timeout(timeout_budget, async {
                let route_id_blob = peer_repo.get_route_id_blob().await?;
                group
                    .iroh_blobs
                    .download_file_from(route_id_blob, hash)
                    .await
            })
            .await;

            match download_result {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(e)) => {
                    let detail = format!("Unable to download hash {hash} from peer {peer_id}: {e}");
                    log_info!(TAG, "{}", detail);
                    last_error = Some(detail);
                }
                Err(_) => {
                    let detail = format!(
                        "Timed out downloading hash {hash} from peer {peer_id} after {}ms",
                        timeout_budget.as_millis()
                    );
                    log_info!(TAG, "{}", detail);
                    last_error = Some(detail);
                }
            }
        }

        if attempt < MEDIA_DOWNLOAD_MAX_ATTEMPTS {
            let backoff = MEDIA_DOWNLOAD_INITIAL_BACKOFF * attempt;
            if let Some(remaining) = MEDIA_DOWNLOAD_OVERALL_TIMEOUT.checked_sub(started.elapsed()) {
                tokio::time::sleep(std::cmp::min(backoff, remaining)).await;
            }
        }
    }

    let detail = last_error.unwrap_or_else(|| "no peer attempts completed".to_string());
    Err(anyhow::anyhow!(
        "Unable to download hash {} from any peer after {} media attempts; last error: {}",
        hash,
        MEDIA_DOWNLOAD_MAX_ATTEMPTS,
        detail
    )
    .into())
}

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

async fn handle_file_stream(
    mut file_data: impl Stream<Item = Result<Bytes, io::Error>> + Unpin,
) -> AppResult<(usize, Bytes)> {
    let mut buffer = BytesMut::new();
    let mut length = 0;

    while let Some(chunk_result) = file_data.next().await {
        let chunk = chunk_result.map_err(|e| AppError(anyhow::Error::new(e)))?;
        buffer.extend_from_slice(&chunk);
        length += chunk.len();
    }

    let final_buffer = web::block(move || buffer.freeze()).await?;

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

    if !repo.can_write() {
        match repo.get_hash_from_dht().await {
            Ok(hash) => {
                if !group.has_hash(&hash).await? {
                    download_hash_for_media(group.as_ref(), &hash).await?;
                }
            }
            Err(err) => {
                log_info!(
                    TAG,
                    "Repo {} has no published collection hash while listing media; returning empty list: {}",
                    repo_id,
                    err
                );
                return Ok(HttpResponse::Ok().json(json!({ "files": [] })));
            }
        }
    }

    // List files and check if they are downloaded
    let files = repo.list_files().await?;
    let mut files_with_status = Vec::new();

    for file_name in files {
        let file_hash = match repo.get_file_hash(&file_name).await {
            Ok(hash) => hash,
            Err(_) => continue, // Handle the error or skip the file if there's an issue
        };
        let is_downloaded = group.has_hash(&file_hash).await.unwrap_or(false); // Check if the file is local
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
            download_hash_for_media(group.as_ref(), &collection_hash).await?;
        }
    }

    // Get the file hash
    let file_hash = repo.get_file_hash(file_name).await?;

    if !group.has_hash(&file_hash).await? {
        download_hash_for_media(group.as_ref(), &file_hash).await?;
    }
    // Trigger file download from peers using the hash
    let file_data = repo.get_file_stream(file_name).await?;

    let (encrypted_length, buffered_data) = handle_file_stream(file_data).await?;

    // Decrypt the file data
    let (decrypted_data, was_encrypted) = repo
        .decrypt_file_data(&buffered_data)
        .map_err(|e| AppError(anyhow::Error::msg(format!("Failed to decrypt file: {e}"))))?;

    if was_encrypted {
        log_info!(
            TAG,
            "File decrypted: {} bytes → {} bytes",
            encrypted_length,
            decrypted_data.len()
        );
    }

    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header((header::CONTENT_LENGTH, decrypted_data.len()))
        .body(decrypted_data))
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

    let file_hash = repo
        .get_file_hash(file_name)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get file hash: {e}"))?;

    Ok(HttpResponse::Ok().json(json!({
        "name": file_name,
        "updated_collection_hash": updated_collection_hash,
        "file_hash": file_hash,
    })))
}
