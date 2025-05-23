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
    use server::{get_backend, init_backend, status, health, BACKEND};
    use tmpdir::TmpDir;
    use base64_url::base64;
    use base64_url::base64::Engine;
    use env_logger;
    use save_dweb_backend::backend::Backend;
    use veilid_core::VeilidUpdate;
    use serial_test::serial;

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

    // Helper: Wait for public internet readiness with timeout and retries
    async fn wait_for_public_internet_ready(backend: &Backend) -> anyhow::Result<()> {
        let mut rx = backend.subscribe_updates().await.ok_or_else(|| anyhow::anyhow!("No update receiver"))?;
        
        // Use a shorter timeout for tests (10 seconds)
        let timeout = if cfg!(test) {
            Duration::from_secs(10)
        } else {
            Duration::from_secs(30)
        };
        
        log::info!("Waiting for public internet to be ready (timeout: {:?})", timeout);
        
        // Try up to 3 times with exponential backoff
        let mut retry_count = 0;
        let max_retries = 3;
        
        while retry_count < max_retries {
            match tokio::time::timeout(timeout, async {
                while let Ok(update) = rx.recv().await {
                    match &update {
                        VeilidUpdate::Attachment(attachment_state) => {
                            log::debug!("Veilid attachment state: {:?}", attachment_state);
                            if attachment_state.public_internet_ready {
                                log::info!("Public internet is ready!");
                                return Ok(());
                            }
                        }
                        _ => log::trace!("Received Veilid update: {:?}", update),
                    }
                }
                Err(anyhow::anyhow!("Update channel closed before network was ready"))
            }).await {
                Ok(result) => return result,
                Err(_) => {
                    retry_count += 1;
                    if retry_count < max_retries {
                        let backoff = Duration::from_secs(2u64.pow(retry_count as u32));
                        log::warn!("Timeout waiting for public internet (attempt {}/{})", retry_count, max_retries);
                        log::info!("Retrying in {:?}...", backoff);
                        tokio::time::sleep(backoff).await;
                        // Resubscribe to get a fresh update channel
                        rx = backend.subscribe_updates().await.ok_or_else(|| anyhow::anyhow!("No update receiver"))?;
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("Failed to establish public internet connection after {} attempts", max_retries))
    }

    #[actix_web::test]
    #[serial]
    async fn basic_test() -> Result<()> {
        let path = TmpDir::new("save-rust-test").await?;

        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));

        {
            let backend = get_backend().await?;

            backend.start().await.expect("Backend failed to start");
        }

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

        {
            let backend = get_backend().await?;

            backend.stop().await.expect("Backend failed to start");
        }

        Ok(())
    }
    #[actix_web::test]
    #[serial]
    async fn test_upload_list_delete() -> Result<()> {
        // Initialize the app
        let path = TmpDir::new("test_api_repo_file_operations").await?;

        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));

        {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
        }

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
            .uri(&format!("/api/groups/{}/repos", group_id))
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
                "/api/groups/{}/repos/{}/media/{}",
                group_id, repo_id, file_name
            ))
            .set_payload(file_content.to_vec())
            .to_request();
        let upload_resp = test::call_service(&app, upload_req).await;

        // Check upload success
        assert!(upload_resp.status().is_success(), "File upload failed");

        // Step 4: List files in the repository
        let list_files_req = test::TestRequest::get()
            .uri(&format!("/api/groups/{}/repos/{}/media", group_id, repo_id))
            .to_request();
        let list_files_resp: FilesResponse =
            test::call_and_read_body_json(&app, list_files_req).await;

        println!("List files response: {:?}", list_files_resp);

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
                "/api/groups/{}/repos/{}/media/{}",
                group_id, repo_id, file_name
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
            .uri(&format!("/api/groups/{}/repos/{}/media", group_id, repo_id))
            .to_request();
        let list_files_after_deletion_resp: FilesResponse =
            test::call_and_read_body_json(&app, list_files_after_deletion_req).await;

        assert!(
            list_files_after_deletion_resp.files.is_empty(),
            "File list should be empty after file deletion"
        );

        // Clean up: Stop the backend
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_join_group() -> Result<()> {
        // Initialize the app
        let path = TmpDir::new("test_api_repo_file_operations").await?;

        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
        }

        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        let store2 = iroh_blobs::store::fs::Store::load(path.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path.to_path_buf().join("test2").as_path(),
            "test2".to_string(),
        )
        .await?;
        let backend2 = Backend::from_dependencies(
            &path.to_path_buf(),
            veilid_api2.clone(),
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

        backend2.stop().await?;
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_replicate_group() -> Result<()> {
        // Initialize the app
        let path = TmpDir::new("test_api_repo_file_operations").await?;

        let store2 = iroh_blobs::store::fs::Store::load(path.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path.to_path_buf().join("test2").as_path(),
            "test2".to_string(),
        )
        .await?;
        let backend2 = Backend::from_dependencies(
            &path.to_path_buf(),
            veilid_api2.clone(),
            update_rx2,
            store2,
        )
        .await
        .unwrap();

        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
        }

        let mut group = backend2.create_group().await?;

        let join_url = group.get_url();

        group.set_name(TEST_GROUP_NAME).await?;

        let repo = group.create_repo().await?;
        repo.set_name(TEST_GROUP_NAME).await?;

        // Step 3: Upload a file to the repository
        let file_name = "example.txt";
        let file_content = b"Test content for file upload";

        repo.upload(&file_name, file_content.to_vec()).await?;

        tokio::time::sleep(Duration::from_secs(2)).await;

        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        {
            let backend = get_backend().await?;
            backend.join_from_url(join_url.as_str()).await?;
        }

        // Add delay to ensure proper synchronization after joining
        tokio::time::sleep(Duration::from_secs(5)).await;

        let get_file_req = test::TestRequest::get()
            .uri(&format!(
                "/api/groups/{}/repos/{}/media/{}",
                group.id().to_string(),
                repo.id().to_string(),
                file_name
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

        // Clean up
        backend2.stop().await?;
        tokio::time::sleep(Duration::from_secs(5)).await;

        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }
        // Add delay to allow tasks to complete
        tokio::time::sleep(Duration::from_secs(2)).await;
        veilid_api2.shutdown().await;

        Ok(())
    }    

    #[actix_web::test]
    #[serial]
    async fn test_refresh_nonexistent_group() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of non-existent group");

        // Initialize the app with basic setup
        let path = TmpDir::new("test_refresh_nonexistent").await?;
        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        let veilid_api = {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
            backend.get_veilid_api().await.unwrap()
        };

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
            .uri(&format!("/api/groups/{}/refresh", fake_group_id))
            .to_request();
        let non_existent_resp = test::call_service(&app, non_existent_req).await;
        assert!(non_existent_resp.status().is_client_error(), "Should return error for non-existent group");

        // Clean up
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }
        // Add delay to allow tasks to complete
        tokio::time::sleep(Duration::from_secs(2)).await;
        veilid_api.shutdown().await;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_refresh_empty_group() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of empty group");

        // Initialize the app with basic setup
        let path = TmpDir::new("test_refresh_empty").await?;
        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        let veilid_api = {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
            backend.get_veilid_api().await.unwrap()
        };

        // Create an empty group
        let empty_group = {
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
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }
        // Add delay to allow tasks to complete
        tokio::time::sleep(Duration::from_secs(2)).await;
        veilid_api.shutdown().await;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_refresh_group_with_single_repo() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of group with single repo");

        // Initialize the app with basic setup
        let path = TmpDir::new("test_refresh_single_repo").await?;
        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        
        // Start backend and wait for public internet readiness
        let veilid_api = {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
            log::info!("Waiting for public internet readiness...");
            wait_for_public_internet_ready(&backend).await?;
            log::info!("Public internet is ready");
            backend.get_veilid_api().await.unwrap()
        };

        // Create a group with a repo and upload a dummy file
        let (group, repo, dummy_file_name, dummy_file_content) = {
            let backend = get_backend().await?;
            let mut group = backend.create_group().await?;
            group.set_name(TEST_GROUP_NAME).await?;
            log::info!("Created group with name: {}", TEST_GROUP_NAME);
            
            let repo = group.create_repo().await?;
            repo.set_name("Test Repo").await?;
            log::info!("Created repo with name: Test Repo");
            
            // Upload a dummy file to ensure the repo has a collection/hash
            let dummy_file_name = "dummy.txt";
            let dummy_file_content = b"dummy content".to_vec();
            repo.upload(dummy_file_name, dummy_file_content.clone()).await?;
            log::info!("Uploaded dummy file: {}", dummy_file_name);
            
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
        log::info!("Refresh response: {:?}", refresh_data);
        
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

        // Clean up with proper delays
        log::info!("Cleaning up test resources...");
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }
        // Add delay to allow tasks to complete
        tokio::time::sleep(Duration::from_secs(2)).await;
        veilid_api.shutdown().await;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_refresh_group_with_file() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of group with file");

        // Initialize the app with basic setup
        let path = TmpDir::new("test_refresh_with_file").await?;
        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        let veilid_api = {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
            backend.get_veilid_api().await.unwrap()
        };

        // Create a group with a repo and upload a file
        let (group, repo) = {
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
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }
        // Add delay to allow tasks to complete
        tokio::time::sleep(Duration::from_secs(2)).await;
        veilid_api.shutdown().await;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_refresh_joined_group() -> Result<()> {
        // Initialize logging
        let _ = env_logger::try_init();
        log::info!("Testing refresh of joined group");

        // Initialize the app with basic setup
        let path = TmpDir::new("test_refresh_joined").await?;

        // Initialize backend2 (creator) first
        let store2 = iroh_blobs::store::fs::Store::load(path.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path.to_path_buf().join("test2").as_path(),
            "test2".to_string(),
        )
        .await?;
        let backend2 = Backend::from_dependencies(
            &path.to_path_buf(),
            veilid_api2.clone(),
            update_rx2,
            store2,
        )
        .await
        .unwrap();

        // Create group and repo in backend2 (without an explicit start or wait_for_public_internet_ready)
        let mut group = backend2.create_group().await?;
        group.set_name(TEST_GROUP_NAME).await?;
        let repo = group.create_repo().await?;
        repo.set_name("Test Repo").await?;

        // Upload a file (using backend2) to ensure repo has a collection/hash
        let file_name = "test.txt";
        let file_content = b"Test content for joined group";
        repo.upload(file_name, file_content.to_vec()).await?;
        log::info!("Uploaded test file to creator's repo");

        // Wait for DHT propagation (after upload, before global BACKEND is initialized)
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Initialize and start the global BACKEND (joiner) (with a wait_for_public_internet_ready)
        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
            log::info!("Waiting for public internet readiness for global BACKEND...");
            wait_for_public_internet_ready(&backend).await?;
            log::info!("Public internet is ready for global BACKEND");
        }

        // Join the group (using the global BACKEND)
        {
            let backend = get_backend().await?;
            backend.join_from_url(group.get_url().as_str()).await?;
            log::info!("Successfully joined group");
        }

        // Wait for replication (after joining, before refresh endpoint is called)
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Initialize app for API testing
        let app = test::init_service(
            App::new()
                .service(status)
                .service(health)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Test refresh endpoint (after joining and waiting)
        log::info!("Testing refresh endpoint for joined group");
        let refresh_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/refresh", group.id()))
            .to_request();
        let refresh_resp = test::call_service(&app, refresh_req).await;
        // Verify response status and content
        assert!(refresh_resp.status().is_success(), "Refresh should succeed");
        let refresh_data: serde_json::Value = test::read_body_json(refresh_resp).await;
        log::info!("Refresh response: {:?}", refresh_data);
        assert_eq!(refresh_data["status"], "success", "Response should indicate success");
        let repos = refresh_data["repos"].as_array().expect("repos should be an array");
        assert_eq!(repos.len(), 1, "Should have one repo");
        let repo_data = &repos[0];
        assert!(repo_data["repo_hash"].is_string(), "repo should have a hash");
        assert_eq!(repo_data["name"], "Test Repo", "repo name should match");
        
        // Verify files from the FIRST refresh
        let refreshed_files_first = repo_data["refreshed_files"].as_array()
            .expect("refreshed_files should be an array for first refresh");
        assert_eq!(refreshed_files_first.len(), 1, "One file should be refreshed on initial sync");
        assert_eq!(refreshed_files_first[0].as_str().unwrap(), file_name, "The correct file should be in refreshed_files on initial sync");
        
        let all_files_first = repo_data["all_files"].as_array().expect("all_files should be an array for first refresh");
        assert_eq!(all_files_first.len(), 1, "Should have one file in all_files on first refresh");
        assert_eq!(all_files_first[0].as_str().unwrap(), file_name, "all_files should contain the uploaded file on first refresh");

        // Verify file is accessible (after first refresh)
        let get_file_req_first = test::TestRequest::get()
            .uri(&format!(
                "/api/groups/{}/repos/{}/media/{}",
                group.id(), repo.id(), file_name
            ))
            .to_request();
        let get_file_resp_first = test::call_service(&app, get_file_req_first).await;
        assert!(get_file_resp_first.status().is_success(), "File should be accessible after first refresh");
        let got_content_first = test::read_body(get_file_resp_first).await;
        assert_eq!(got_content_first.to_vec(), file_content.to_vec(), "File content should match after first refresh");

        // ---- SECOND REFRESH ----
        log::info!("Testing second refresh endpoint for joined group (should be no-op)");
        let refresh_req_second = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/refresh", group.id()))
            .to_request();
        let refresh_resp_second = test::call_service(&app, refresh_req_second).await;
        assert!(refresh_resp_second.status().is_success(), "Second refresh should succeed");
        let refresh_data_second: serde_json::Value = test::read_body_json(refresh_resp_second).await;
        assert_eq!(refresh_data_second["status"], "success", "Second refresh response should indicate success");
        let repos_second = refresh_data_second["repos"].as_array().expect("repos should be an array for second refresh");
        assert_eq!(repos_second.len(), 1, "Should have one repo in second refresh");
        let repo_data_second = &repos_second[0];

        let refreshed_files_second = repo_data_second["refreshed_files"].as_array()
            .expect("refreshed_files should be an array for second refresh");
        assert!(refreshed_files_second.is_empty(), "No files should be refreshed on second sync as all are present");

        let all_files_second = repo_data_second["all_files"].as_array().expect("all_files should be an array for second refresh");
        assert_eq!(all_files_second.len(), 1, "Should still have one file in all_files on second refresh");
        assert_eq!(all_files_second[0].as_str().unwrap(), file_name, "all_files should still contain the uploaded file on second refresh");

        // Clean up (stop backend2, stop global BACKEND, shutdown veilid_api2)
        log::info!("Cleaning up test resources...");
        backend2.stop().await?;
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        veilid_api2.shutdown().await;

        Ok(())
    }

    #[actix_web::test]
    #[serial]
    async fn test_health_endpoint() -> Result<()> {
        // Initialize the app
        let path = TmpDir::new("test-health-endpoint").await?;

        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));

        {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
        }

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
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }

        Ok(())
    }
}
