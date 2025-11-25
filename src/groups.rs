use actix_web::{web, delete, get, post, Responder, HttpResponse};
use serde_json::json;
use crate::error::AppResult;
use crate::log_debug;
use crate::models::{IntoSnowbirdGroupsWithNames, RequestName, RequestUrl, SnowbirdGroup};
use crate::repos;
use crate::constants::{TAG};

use crate::server::get_backend;
use crate::utils::create_veilid_cryptokey_from_base64;
use save_dweb_backend::common::DHTEntity;

pub fn scope() -> actix_web::Scope {
    web::scope("/groups")
        .service(get_groups)
        .service(create_group)
        .service(join_group_from_url)
        .service(
            web::scope("/{group_id}")
                .service(delete_group)
                .service(get_group)
                .service(refresh_group)
                .service(repos::scope())
        )
}

// This doesn't seem to be the way to delete a group.
//
#[delete("")]
async fn delete_group(group_id: web::Path<String>) -> AppResult<impl Responder> {
    let backend = get_backend().await?;
    let group_id = group_id.into_inner();
    let crypto_key = create_veilid_cryptokey_from_base64(&group_id)?;
    
    backend.close_group(crypto_key).await?;

    Ok(HttpResponse::Ok().json(json!({})))
}

#[get("")]
async fn get_groups() -> AppResult<impl Responder> {
    let backend = get_backend().await?;
    let groups = backend.list_groups().await.unwrap();
    let snowbird_groups = groups.into_snowbird_groups_with_names().await;

    Ok(HttpResponse::Ok().json(json!({ "groups": snowbird_groups })))
}

#[get("")]
async fn get_group(group_id: web::Path<String>) -> AppResult<impl Responder> {
    let backend = get_backend().await?;
    log_debug!(TAG, "got backend");

    let group_id = group_id.into_inner();
    let key = create_veilid_cryptokey_from_base64(group_id.as_str()).unwrap();
    log_debug!(TAG, "got key {}", key);

    let backend_group = backend.get_group(&key).await?;
    log_debug!(TAG, "got backend group");

    let mut snowbird_group: SnowbirdGroup = backend_group.as_ref().into();
    log_debug!(TAG, "got snowbird group");

    snowbird_group.fill_name(backend_group.as_ref()).await;

    Ok(HttpResponse::Ok().json(snowbird_group))
}

#[post("")]
async fn create_group(request_name: web::Json<RequestName>) -> AppResult<impl Responder> {
    let request = request_name.into_inner();

    log_debug!(TAG, "got body {:?}", request);

    let backend = get_backend().await?;
    log_debug!(TAG, "got backend");

    let backend_group = backend.create_group().await?;
    log_debug!(TAG, "got backend group");
    log_debug!(TAG, "backend url = {}", backend_group.get_url());

    // Set group name using the request
    backend_group.set_name(&request.name).await?;

    let mut snowbird_group: SnowbirdGroup = (&backend_group).into();
    log_debug!(TAG, "got snowbird group");

    snowbird_group.name = Some(request.name);

    Ok(HttpResponse::Ok().json(snowbird_group))
}

#[post("/join_from_url")]
async fn join_group_from_url(request_url: web::Json<RequestUrl>) -> AppResult<impl Responder> {
    let request = request_url.into_inner();

    log_debug!(TAG, "Received request with URL: {:?}", request.url);

    let backend = get_backend().await?;
    log_debug!(TAG, "Obtained backend instance");

    let backend_group = backend.join_from_url(&request.url).await?;
    log_debug!(TAG, "Joined backend group successfully");

    let mut snowbird_group: SnowbirdGroup = (&*backend_group).into();
    log_debug!(TAG, "Converted to SnowbirdGroup");

    snowbird_group.fill_name(backend_group.as_ref()).await;
    log_debug!(TAG, "Filled group name");

    Ok(HttpResponse::Ok().json(snowbird_group))
}

#[post("/refresh")]
async fn refresh_group(group_id: web::Path<String>) -> AppResult<impl Responder> {
    let backend = get_backend().await?;
    log_debug!(TAG, "Starting group refresh");

    let group_id = group_id.into_inner();
    let key = create_veilid_cryptokey_from_base64(group_id.as_str())?;
    log_debug!(TAG, "Got key {}", key);

    // Return error if group not found
    let group = match backend.get_group(&key).await {
        Ok(group) => group,
        Err(e) => {
            return Ok(HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": format!("Group not found: {}", e)
            })));
        }
    };
    log_debug!(TAG, "Got group");

    // Get all repos in the group
    let locked_repos = group.repos.lock().await;
    let repos: Vec<_> = locked_repos.values().cloned().collect();
    drop(locked_repos); // Release the lock before async operations

    // Return empty arrays if no repos
    if repos.is_empty() {
        return Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "refreshed_files": [],
            "repos": []
        })));
    }

    let mut refreshed_repos = Vec::new();

    // For each repo, refresh its collection and files
    for repo in repos {
        log_debug!(TAG, "Refreshing repo {}", repo.id());

        let mut repo_info = json!({
            "repo_id": repo.id().to_string(),
            "can_write": repo.can_write(),
            "name": repo.get_name().await.unwrap_or_default(),
            "refreshed_files": json!(Vec::<String>::new()), // Initialize empty
            "all_files": json!(Vec::<String>::new())       // Initialize empty
        });
        let mut refreshed_files_vec = Vec::new();
        let mut all_files_vec: Vec<String> = Vec::new();

        // Get current repo hash and collection info
        match repo.get_hash_from_dht().await {
            Ok(repo_hash) => {
                repo_info["repo_hash"] = json!(repo_hash.to_string());
                
                // Refresh collection hash if needed
                log_debug!(TAG, "Repo {} has DHT hash {}. Checking if group has it locally.", repo.id(), repo_hash);
                if !group.has_hash(&repo_hash).await? {
                    log_debug!(TAG, "Repo {} collection {} not found locally. Downloading...", repo.id(), repo_hash);
                    match group.download_hash_from_peers(&repo_hash).await {
                        Ok(_) => {
                            log_debug!(TAG, "Successfully downloaded collection hash {} for repo {}", repo_hash, repo.id());
                        }
                        Err(e) => {
                            log_debug!(TAG, "Error downloading collection hash {} for repo {}: {}", repo_hash, repo.id(), e);
                            repo_info["error"] = json!(format!("Error downloading collection: {}", e));
                            refreshed_repos.push(repo_info);
                            continue; // Skip to next repo if download fails
                        }
                    }
                } else {
                    log_debug!(TAG, "Repo {} collection {} already local.", repo.id(), repo_hash);
                }

                // Now that the collection is ensured to be local, list all files in the repo
                match repo.list_files().await {
                    Ok(files) => {
                        log_debug!(TAG, "Repo {} lists files: {:?}", repo.id(), files);
                        all_files_vec = files;
                    }
                    Err(e) => {
                        log_debug!(TAG, "Error listing files for repo {} after ensuring collection download: {}", repo.id(), e);
                        // Even if listing fails here, we might have a repo_hash, so continue with empty files.
                        // Or, handle as a more significant error. For now, log and continue.
                        repo_info["error_listing_files"] = json!(format!("Error listing files post-download: {}", e));
                    }
                };
                repo_info["all_files"] = json!(all_files_vec.clone());


                // For each file, check if it needs to be refreshed
                for file_name in &all_files_vec {
                    match repo.get_file_hash(file_name).await {
                        Ok(file_hash) => {
                            if !group.has_hash(&file_hash).await? {
                                log_debug!(TAG, "File {} hash {} not found locally. Downloading...", file_name, file_hash);
                                match group.download_hash_from_peers(&file_hash).await {
                                    Ok(_) => {
                                        log_debug!(TAG, "Successfully downloaded file hash {} for {}", file_hash, file_name);
                                        refreshed_files_vec.push(file_name.clone());
                                    }
                                    Err(e) => {
                                        log_debug!(TAG, "Error downloading file {} hash {}: {}", file_name, file_hash, e);
                                        // Optionally add to a list of files that failed to download
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log_debug!(TAG, "Error getting hash for file {}: {}", file_name, e);
                        }
                    }
                }
                repo_info["refreshed_files"] = json!(refreshed_files_vec);
            }
            Err(e) => {
                log_debug!(TAG, "Error getting repo hash for {}: {}", repo.id(), e);
                repo_info["error"] = json!(format!("Error getting repo hash from DHT: {}", e));
            }
        }

        refreshed_repos.push(repo_info);
    }

    Ok(HttpResponse::Ok().json(json!({
        "status": "success",
        "repos": refreshed_repos
    })))
}
