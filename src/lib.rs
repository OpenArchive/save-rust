#![recursion_limit = "256"]

#[cfg(target_os = "android")]
pub mod android_bridge;
#[cfg(target_os = "android")]
pub mod jni_globals;

pub mod actix_route_dumper;
pub mod constants;
pub mod error;
pub mod logging;

pub mod groups;
pub mod media;
pub mod models;
pub mod repos;
pub mod server;
pub mod utils;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use actix_web::{test, web, App};
    use anyhow::Result;
    use models::{RequestName, RequestUrl, SnowbirdFile, SnowbirdGroup, SnowbirdRepo};
    use save_dweb_backend::{common::DHTEntity, constants::TEST_GROUP_NAME};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use server::{status, health, set_backend, clear_backend};
    use tmpdir::TmpDir;
    use base64_url::base64;
    use base64_url::base64::Engine;
    use save_dweb_backend::backend::Backend;
    use veilid_core::VeilidUpdate;
    use serial_test::serial;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;

    #[derive(Debug, Serialize, Deserialize)]
    struct GroupsResponse {
        groups: Vec<SnowbirdGroup>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct ReposResponse {
        repos: Vec<SnowbirdRepo>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct FilesResponse {
        files: Vec<SnowbirdFile>,
    }

    /// Helper function to generate unique test config
    /// Returns (TmpDir, namespace_string)
    async fn get_test_config(test_name: &str) -> (TmpDir, String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let namespace = format!("save-rust-{test_name}-{timestamp}");
        let path = TmpDir::new(test_name).await.unwrap();
        (path, namespace)
    }

    /// Helper to initialize Backend with unique namespace
    async fn init_test_backend(test_name: &str) -> Result<TmpDir> {
        // Clear any previous backend
        clear_backend()?;
        
        let (path, namespace) = get_test_config(test_name).await;

        let store = iroh_blobs::store::fs::Store::load(path.to_path_buf().join("iroh")).await?;
        let (veilid_api, update_rx) = save_dweb_backend::common::init_veilid(
            &path.to_path_buf(),
            namespace,
        )
        .await?;

        let backend = Backend::from_dependencies(
            &path.to_path_buf(),
            veilid_api,
            update_rx,
            store,
        )
        .await?;
        
        // Set the BACKEND static so routes can access it
        set_backend(Arc::new(TokioMutex::new(backend)))?;
        
        Ok(path)
    }

    // Helper: Wait for public internet readiness with timeout and retries
    async fn wait_for_public_internet_ready(backend: &Backend) -> anyhow::Result<()> {
        let mut rx = backend.subscribe_updates().await.ok_or_else(|| anyhow::anyhow!("No update receiver"))?;
        
        let timeout = if cfg!(test) {
            Duration::from_secs(15)  
        } else {
            Duration::from_secs(30)
        };
        
        log::info!("Waiting for public internet to be ready (timeout: {timeout:?})");
        
        // Try up to 6 times with exponential backoff
        let mut retry_count = 0;
        let max_retries = 6;  
        
        while retry_count < max_retries {
            match tokio::time::timeout(timeout, async {
                while let Ok(update) = rx.recv().await {
                    match &update {
                        VeilidUpdate::Attachment(attachment_state) => {
                            log::debug!("Veilid attachment state: {attachment_state:?}");
                            if attachment_state.public_internet_ready {
                                log::info!("Public internet is ready!");
                                return Ok(());
                            }
                        }
                        _ => log::trace!("Received Veilid update: {update:?}"),
                    }
                }
                Err(anyhow::anyhow!("Update channel closed before network was ready"))
            }).await {
                Ok(result) => return result,
                Err(_) => {
                    retry_count += 1;
                    if retry_count < max_retries {
                        let backoff = Duration::from_secs(2u64.pow(retry_count as u32));
                        log::warn!("Timeout waiting for public internet (attempt {retry_count}/{max_retries})");
                        log::info!("Retrying in {backoff:?}...");
                        tokio::time::sleep(backoff).await;
                        // Resubscribe to get a fresh update channel
                        rx = backend.subscribe_updates().await.ok_or_else(|| anyhow::anyhow!("No update receiver"))?;
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("Failed to establish public internet connection after {max_retries} attempts"))
    }

    // Helper function to properly clean up test resources
    async fn cleanup_test_resources() -> Result<()> {
        // Get the backend and stop it
        use server::get_backend;
        if let Ok(backend) = get_backend().await {
            backend.stop().await?;
        }
        
        // Clear the backend static
        clear_backend()?;
        
        // Add a small delay to ensure everything is cleaned up
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn basic_test() -> Result<()> {
        let _path = init_test_backend("basic_test").await?;

        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        let req = test::TestRequest::default().uri("/api/groups").to_request();
        let resp: GroupsResponse = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.groups.len(), 0);

        let req = test::TestRequest::post()
            .uri("/api/groups")
            .set_json(RequestName {
                name: "example".to_string(),
            })
            .to_request();
        let group: SnowbirdGroup = test::call_and_read_body_json(&app, req).await;

        assert_eq!(group.name, Some("example".to_string()));

        let req = test::TestRequest::default()
            .uri(format!("/api/groups/{}/repos", group.key).as_str())
            .to_request();
        let resp: ReposResponse = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.repos.len(), 0, "Should have no repos at first");

        let req = test::TestRequest::post()
            .uri(format!("/api/groups/{}/repos", group.key).as_str())
            .set_json(RequestName {
                name: "example repo".to_string(),
            })
            .to_request();
        let repo: SnowbirdRepo = test::call_and_read_body_json(&app, req).await;

        assert_eq!(repo.name, "example repo".to_string());

        let req = test::TestRequest::default()
            .uri(format!("/api/groups/{}/repos", group.key).as_str())
            .to_request();
        let resp: ReposResponse = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.repos.len(), 1, "Should have 1 repo after create");

        cleanup_test_resources().await?;

        Ok(())
    }
    #[actix_web::test]
    #[serial]
    async fn test_upload_list_delete() -> Result<()> {
        // Initialize the app
        let _path = init_test_backend("test_upload_list_delete").await?;

        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Step 1: Create a group via the API
        let create_group_req = test::TestRequest::post()
            .uri("/api/groups")
            .set_json(json!({ "name": "Test Group" }))
            .to_request();
        let create_group_resp: serde_json::Value =
            test::call_and_read_body_json(&app, create_group_req).await;
        let group_id = create_group_resp["key"]
            .as_str()
            .expect("No group key returned");

        // Step 2: Create a repo via the API
        let create_repo_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{group_id}/repos"))
            .set_json(json!({ "name": "Test Repo" }))
            .to_request();
        let create_repo_resp: serde_json::Value =
            test::call_and_read_body_json(&app, create_repo_req).await;

        let repo_id = create_repo_resp["key"]
            .as_str()
            .expect("No repo key returned");

        // Step 3: Upload a file to the repository
        let file_name = "example.txt";
        let file_content = b"Test content for file upload";

        let upload_req = test::TestRequest::post()
            .uri(&format!(
                "/api/groups/{group_id}/repos/{repo_id}/media/{file_name}"
            ))
            .set_payload(file_content.to_vec())
            .to_request();
        let upload_resp = test::call_service(&app, upload_req).await;

        // Check upload success
        assert!(upload_resp.status().is_success(), "File upload failed");

        // Step 4: List files in the repository
        let list_files_req = test::TestRequest::get()
            .uri(&format!("/api/groups/{group_id}/repos/{repo_id}/media"))
            .to_request();
        let list_files_resp: FilesResponse =
            test::call_and_read_body_json(&app, list_files_req).await;

        println!("List files response: {list_files_resp:?}");

        // Now check if the response is an array directly
        let files_array = list_files_resp.files;
        assert_eq!(files_array.len(), 1, "There should be one file in the repo");

        // Ensure the file name matches what we uploaded
        let file_obj = &files_array[0];
        assert_eq!(
            file_obj.name, "example.txt",
            "The listed file should match the uploaded file"
        );

        let get_file_req = test::TestRequest::get()
            .uri(&format!(
                "/api/groups/{group_id}/repos/{repo_id}/media/{file_name}"
            ))
            .to_request();
        let get_file_resp = test::call_service(&app, get_file_req).await;
        assert!(get_file_resp.status().is_success(), "File download failed");

        let got_file_data = test::read_body(get_file_resp).await;
        assert_eq!(
            got_file_data.to_vec().as_slice(),
            file_content,
            "Downloaded back file content"
        );

        // Step 5: Delete the file from the repository
        let delete_file_req = test::TestRequest::delete()
            .uri(&format!(
                "/api/groups/{}/repos/{}/media/{}",
                group_id, repo_id, "example.txt"
            ))
            .to_request();
        let delete_resp = test::call_service(&app, delete_file_req).await;

        assert!(delete_resp.status().is_success(), "File deletion failed");

        // Step 6: Verify the file is no longer listed
        let list_files_after_deletion_req = test::TestRequest::get()
            .uri(&format!("/api/groups/{group_id}/repos/{repo_id}/media"))
            .to_request();
        let list_files_after_deletion_resp: FilesResponse =
            test::call_and_read_body_json(&app, list_files_after_deletion_req).await;

        assert!(
            list_files_after_deletion_resp.files.is_empty(),
            "File list should be empty after file deletion"
        );

        // Clean up
        cleanup_test_resources().await?;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_join_group() -> Result<()> {
        // Initialize main backend
        let _path = init_test_backend("test_join_group_main").await?;

        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Initialize secondary backend with unique namespace
        let (path2, namespace2) = get_test_config("test_join_group_secondary").await;
        let store2 = iroh_blobs::store::fs::Store::load(path2.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path2.to_path_buf().as_path(),
            namespace2,
        )
        .await?;
        let backend2 = Backend::from_dependencies(
            &path2.to_path_buf(),
            veilid_api2,
            update_rx2,
            store2,
        )
        .await
        .unwrap();

        let mut group = backend2.create_group().await?;

        group.set_name(TEST_GROUP_NAME).await?;

        let repo = group.create_repo().await?;
        repo.set_name(TEST_GROUP_NAME).await?;

        // Step 1: Create a group via the API
        let join_group_req = test::TestRequest::post()
            .uri("/api/groups/join_from_url")
            .set_json(RequestUrl {
                url: group.get_url(),
            })
            .to_request();
        let join_group_resp = test::call_service(&app, join_group_req).await;

        assert!(join_group_resp.status().is_success());

        let joined_group: SnowbirdGroup = test::read_body_json(join_group_resp).await;

        assert_eq!(
            joined_group.name,
            Some(TEST_GROUP_NAME.to_string()),
            "Joined group has expected name"
        );

        let groups_req = test::TestRequest::default().uri("/api/groups").to_request();
        let groups_resp = test::call_service(&app, groups_req).await;

        assert!(groups_resp.status().is_success(), "Group join successful");

        let groups: GroupsResponse = test::read_body_json(groups_resp).await;

        assert_eq!(groups.groups.len(), 1);

        let req = test::TestRequest::default()
            .uri(format!("/api/groups/{}/repos", joined_group.key).as_str())
            .to_request();
        let resp: ReposResponse = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.repos.len(), 1, "Should have 1 repo after joining");

        // Clean up both backends - secondary first, then main
        backend2.stop().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        cleanup_test_resources().await?;

        Ok(())
    }

    // NOTE: This test is flaky due to P2P peer discovery timing issues.
    // Two backends on the same machine sometimes can't find each other in time.
    // Skipped in CI, but useful for manual testing of P2P replication.
    #[actix_web::test]
    #[serial]
    async fn test_replicate_group() -> Result<()> {
        // Create secondary backend (creator) first
        let (path2, namespace2) = get_test_config("test_replicate_group_secondary").await;
        let store2 = iroh_blobs::store::fs::Store::load(path2.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path2.to_path_buf().as_path(),
            namespace2,
        )
        .await?;
        let backend2 = Backend::from_dependencies(
            &path2.to_path_buf(),
            veilid_api2,
            update_rx2,
            store2,
        )
        .await
        .unwrap();

        // Initialize main backend (joiner)
        let _path = init_test_backend("test_replicate_group_main").await?;
    
        // Create group and repo in backend2 (creator)
        let mut group = backend2.create_group().await?;
        let join_url = group.get_url();
        group.set_name(TEST_GROUP_NAME).await?;
        let repo = group.create_repo().await?;
        repo.set_name(TEST_GROUP_NAME).await?;
    
        // Upload a file to the repository
        let file_name = "example.txt";
        let file_content = b"Test content for file upload";
        repo.upload(file_name, file_content.to_vec()).await?;
    
        tokio::time::sleep(Duration::from_secs(2)).await;
    
        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;
    
        // Join the group using the main backend
        {
            use server::get_backend;
            let backend = get_backend().await?;
            backend.join_from_url(join_url.as_str()).await?;
        }
    
        // Wait for replication to complete
        tokio::time::sleep(Duration::from_secs(2)).await;
    
        // Test HTTP endpoints after replication
        // 1. Verify group exists and has correct name
        let groups_req = test::TestRequest::get().uri("/api/groups").to_request();
        let groups_resp: GroupsResponse = test::call_and_read_body_json(&app, groups_req).await;
        assert_eq!(groups_resp.groups.len(), 1, "Should have one group after joining");
        assert_eq!(groups_resp.groups[0].name, Some(TEST_GROUP_NAME.to_string()), 
            "Group should have correct name");
    
        // 2. Verify repo exists and has correct name
        let repos_req = test::TestRequest::get()
            .uri(&format!("/api/groups/{}/repos", group.id()))
            .to_request();
        let repos_resp: ReposResponse = test::call_and_read_body_json(&app, repos_req).await;
        assert_eq!(repos_resp.repos.len(), 1, "Should have one repo after joining");
        assert_eq!(repos_resp.repos[0].name, TEST_GROUP_NAME, "Repo should have correct name");
    
        // 3. Verify file exists and has correct content
        let file_req = test::TestRequest::get()
            .uri(&format!(
                "/api/groups/{}/repos/{}/media/{}",
                group.id(), repo.id(), file_name
            ))
            .to_request();
        let file_resp = test::call_service(&app, file_req).await;
        assert!(file_resp.status().is_success(), "File should be accessible after replication");
        let got_content = test::read_body(file_resp).await;
        assert_eq!(got_content.to_vec(), file_content.to_vec(),
            "File content should match after replication");

        // Clean up both backends - secondary first, then main
        backend2.stop().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        cleanup_test_resources().await?;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_refresh_nonexistent_group() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of non-existent group");

        // Initialize backend
        let _path = init_test_backend("test_refresh_nonexistent").await?;

        let app = test::init_service(
            App::new()
                .service(status)
                .service(health)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Test refreshing non-existent group
        let fake_group_id = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 32]);
        let non_existent_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{fake_group_id}/refresh"))
            .to_request();
        let non_existent_resp = test::call_service(&app, non_existent_req).await;
        assert!(non_existent_resp.status().is_client_error(), "Should return error for non-existent group");

        // Clean up
        cleanup_test_resources().await?;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_refresh_empty_group() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of empty group");

        // Initialize backend
        let _path = init_test_backend("test_refresh_empty").await?;

        // Create an empty group
        let empty_group = {
            use server::get_backend;
            let backend = get_backend().await?;
            backend.create_group().await?
        };

        let app = test::init_service(
            App::new()
                .service(status)
                .service(health)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Test refreshing empty group
        let empty_group_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/refresh", empty_group.id()))
            .to_request();
        let empty_group_resp = test::call_service(&app, empty_group_req).await;
        assert!(empty_group_resp.status().is_success(), "Should handle empty group");
        let empty_group_data: serde_json::Value = test::read_body_json(empty_group_resp).await;
        assert_eq!(empty_group_data["status"], "success");
        assert!(empty_group_data["repos"].as_array().unwrap().is_empty());

        // Clean up
        cleanup_test_resources().await?;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_refresh_group_with_single_repo() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of group with single repo");

        // Initialize backend
        let _path = init_test_backend("test_refresh_single_repo").await?;

        // Create a group with a repo and upload a dummy file
        let (group, repo, dummy_file_name, dummy_file_content) = {
            use server::get_backend;
            let backend = get_backend().await?;
            
            // Wait for public internet readiness
            log::info!("Waiting for public internet readiness...");
            wait_for_public_internet_ready(&backend).await?;
            log::info!("Public internet is ready");
            
            let mut group = backend.create_group().await?;
            group.set_name(TEST_GROUP_NAME).await?;
            log::info!("Created group with name: {TEST_GROUP_NAME}");
            
            let repo = group.create_repo().await?;
            repo.set_name("Test Repo").await?;
            log::info!("Created repo with name: Test Repo");
            
            // Upload a dummy file to ensure the repo has a collection/hash
            let dummy_file_name = "dummy.txt";
            let dummy_file_content = b"dummy content".to_vec();
            repo.upload(dummy_file_name, dummy_file_content.clone()).await?;
            log::info!("Uploaded dummy file: {dummy_file_name}");
            
            (group, repo, dummy_file_name, dummy_file_content)
        };

        let app = test::init_service(
            App::new()
                .service(status)
                .service(health)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Test refreshing group with single repo
        log::info!("Testing refresh endpoint for group: {}", group.id());
        let refresh_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/refresh", group.id()))
            .to_request();
        let refresh_resp = test::call_service(&app, refresh_req).await;
        
        // Verify response status
        assert!(refresh_resp.status().is_success(), 
            "Refresh should succeed, got status: {}", refresh_resp.status());
        
        // Parse and verify response data
        let refresh_data: serde_json::Value = test::read_body_json(refresh_resp).await;
        log::info!("Refresh response: {refresh_data:?}");
        
        assert_eq!(refresh_data["status"], "success", "Response should indicate success");
        
        // Verify repos array
        let repos = refresh_data["repos"].as_array()
            .expect("repos should be an array in response");
        assert_eq!(repos.len(), 1, "Should have exactly one repo");
        
        // Verify repo details
        let repo_data = &repos[0];
        assert!(repo_data["can_write"].as_bool().unwrap(), "repo should be writable");
        assert!(repo_data["repo_hash"].is_string(), "repo should have a hash");
        assert_eq!(repo_data["name"], "Test Repo", "repo name should match");
        
        // Verify refreshed files
        let refreshed_files = repo_data["refreshed_files"].as_array()
            .expect("refreshed_files should be an array");
        assert!(refreshed_files.is_empty(), "No files should be refreshed since all are present");

        // Verify all_files contains the uploaded file
        let all_files = repo_data["all_files"].as_array().expect("all_files should be an array");
        assert_eq!(all_files.len(), 1, "Should have one file in all_files");
        assert_eq!(all_files[0], dummy_file_name, "all_files should contain the uploaded file");

        // Verify file is accessible after refresh
        let get_file_req = test::TestRequest::get()
            .uri(&format!(
                "/api/groups/{}/repos/{}/media/{}",
                group.id(), repo.id(), dummy_file_name
            ))
            .to_request();
        let get_file_resp = test::call_service(&app, get_file_req).await;
        assert!(get_file_resp.status().is_success(), "File should be accessible after refresh");
        let got_content = test::read_body(get_file_resp).await;
        assert_eq!(got_content.to_vec(), dummy_file_content,
            "File content should match after refresh");

        // Clean up
        log::info!("Cleaning up test resources...");
        cleanup_test_resources().await?;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_refresh_group_with_file() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of group with file");

        // Initialize backend
        let _path = init_test_backend("test_refresh_with_file").await?;

        // Create a group with a repo and upload a file
        let (group, repo) = {
            use server::get_backend;
            let backend = get_backend().await?;
            let mut group = backend.create_group().await?;
            group.set_name(TEST_GROUP_NAME).await?;
            let repo = group.create_repo().await?;
            repo.set_name("Test Repo").await?;
            (group, repo)
        };

        // Upload a file
        let file_name = "test.txt";
        let file_content = b"Test content";
        repo.upload(file_name, file_content.to_vec()).await?;

        let app = test::init_service(
            App::new()
                .service(status)
                .service(health)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Test refreshing group with file
        let refresh_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/refresh", group.id()))
            .to_request();
        let refresh_resp = test::call_service(&app, refresh_req).await;
        assert!(refresh_resp.status().is_success(), "Refresh should succeed");
        let refresh_data: serde_json::Value = test::read_body_json(refresh_resp).await;
        assert_eq!(refresh_data["status"], "success");
        let repos = refresh_data["repos"].as_array().expect("repos should be an array");
        assert_eq!(repos.len(), 1, "Should have one repo");
        let repo_data = &repos[0];
        let refreshed_files = repo_data["refreshed_files"].as_array().expect("refreshed_files should be an array");
        assert!(refreshed_files.is_empty(), "No files should be refreshed since all are present");
        let all_files = repo_data["all_files"].as_array().expect("all_files should be an array");
        assert_eq!(all_files.len(), 1, "Should have one file in all_files");
        assert_eq!(all_files[0], file_name, "all_files should contain the uploaded file");

        // Verify file is accessible
        let get_file_req = test::TestRequest::get()
            .uri(&format!(
                "/api/groups/{}/repos/{}/media/{}",
                group.id(), repo.id(), file_name
            ))
            .to_request();
        let get_file_resp = test::call_service(&app, get_file_req).await;
        assert!(get_file_resp.status().is_success(), "File should be accessible");
        let got_content = test::read_body(get_file_resp).await;
        assert_eq!(got_content.to_vec(), file_content.to_vec(), "File content should match");

        // Clean up
        cleanup_test_resources().await?;

        Ok(())
    }

    // NOTE: This test is flaky due to P2P peer discovery timing issues.
    // Two backends on the same machine sometimes can't find each other in time.
    // Skipped in CI, but useful for manual testing of P2P replication.
    #[actix_web::test]
    #[serial]
    async fn test_refresh_joined_group() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of joined group");

        // Create secondary backend (creator) first
        let (path2, namespace2) = get_test_config("test_refresh_joined_secondary").await;
        let store2 = iroh_blobs::store::fs::Store::load(path2.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path2.to_path_buf().as_path(),
            namespace2,
        )
        .await?;
        let backend2 = Backend::from_dependencies(
            &path2.to_path_buf(),
            veilid_api2,
            update_rx2,
            store2,
        )
        .await
        .unwrap();

        // Initialize main backend (joiner)
        let _path = init_test_backend("test_refresh_joined_main").await?;

        // Create group and repo in backend2 (creator)
        let mut group = backend2.create_group().await?;
        let join_url = group.get_url();
        group.set_name(TEST_GROUP_NAME).await?;
        let repo = group.create_repo().await?;
        repo.set_name(TEST_GROUP_NAME).await?;

        // Upload a file to the repository
        let file_name = "example.txt";
        let file_content = b"Test content for file upload";
        repo.upload(file_name, file_content.to_vec()).await?;

        tokio::time::sleep(Duration::from_secs(2)).await;

        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Join the group using the main backend
        {
            use server::get_backend;
            let backend = get_backend().await?;
            backend.join_from_url(join_url.as_str()).await?;
        }

        // Wait for replication to complete
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Test first refresh - should fetch files from network
        let refresh_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/refresh", group.id()))
            .to_request();
        let refresh_resp = test::call_service(&app, refresh_req).await;
        assert!(refresh_resp.status().is_success(), "First refresh should succeed");
        
        let refresh_data: serde_json::Value = test::read_body_json(refresh_resp).await;
        assert_eq!(refresh_data["status"], "success", "First refresh status should be success");
        
        let repos = refresh_data["repos"].as_array().expect("repos should be an array");
        assert_eq!(repos.len(), 1, "Should have one repo after joining");
        
        let repo_data = &repos[0];
        assert_eq!(repo_data["name"], TEST_GROUP_NAME, "Repo should have correct name");
        
        // First refresh should have refreshed files
        let refreshed_files = repo_data["refreshed_files"].as_array()
            .expect("refreshed_files should be an array");
        assert_eq!(refreshed_files.len(), 1, "Should have refreshed 1 file on first refresh");
        assert_eq!(refreshed_files[0].as_str().unwrap(), file_name, 
            "Should have refreshed the correct file");
        
        let all_files = repo_data["all_files"].as_array().expect("all_files should be an array");
        assert_eq!(all_files.len(), 1, "Should have one file in all_files");
        assert_eq!(all_files[0].as_str().unwrap(), file_name, 
            "all_files should contain the uploaded file");

        // Verify file is accessible after refresh
        let get_file_req = test::TestRequest::get()
            .uri(&format!(
                "/api/groups/{}/repos/{}/media/{}",
                group.id(), repo.id(), file_name
            ))
            .to_request();
        let get_file_resp = test::call_service(&app, get_file_req).await;
        assert!(get_file_resp.status().is_success(), "File should be accessible after refresh");
        let got_content = test::read_body(get_file_resp).await;
        assert_eq!(got_content.to_vec(), file_content.to_vec(), 
            "File content should match after refresh");

        // Test second refresh - should be no-op since all files are present
        let refresh_req2 = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/refresh", group.id()))
            .to_request();
        let refresh_resp2 = test::call_service(&app, refresh_req2).await;
        assert!(refresh_resp2.status().is_success(), "Second refresh should succeed");
        
        let refresh_data2: serde_json::Value = test::read_body_json(refresh_resp2).await;
        assert_eq!(refresh_data2["status"], "success", "Second refresh status should be success");
        
        let repos2 = refresh_data2["repos"].as_array().expect("repos should be an array");
        assert_eq!(repos2.len(), 1, "Should still have one repo");
        
        let repo_data2 = &repos2[0];
        let refreshed_files2 = repo_data2["refreshed_files"].as_array()
            .expect("refreshed_files should be an array");
        assert!(refreshed_files2.is_empty(),
            "No files should be refreshed on second call since all are present");

        // Clean up both backends - secondary first, then main
        backend2.stop().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        cleanup_test_resources().await?;

        Ok(())
    }
    #[actix_web::test]
    #[serial]
    async fn test_health_endpoint() -> Result<()> {
        // Initialize backend with unique namespace
        let _path = init_test_backend("test_health_endpoint").await?;

        let app = test::init_service(
            App::new()
                .service(status)
                .service(health)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Test the health endpoint
        let health_req = test::TestRequest::get().uri("/health").to_request();
        let health_resp = test::call_service(&app, health_req).await;
        
        // Verify response status is 200 OK
        assert!(health_resp.status().is_success(), "Health endpoint should return 200 OK");
        
        // Verify response body
        let health_data: serde_json::Value = test::read_body_json(health_resp).await;
        assert_eq!(health_data["status"], "OK", "Health endpoint should return status OK");

        // Clean up
        cleanup_test_resources().await?;

        Ok(())
    }
}
