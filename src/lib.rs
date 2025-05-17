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
    use server::server::{get_backend, init_backend, status, BACKEND};
    use tmpdir::TmpDir;

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

    #[actix_web::test]
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
        let backend2 = save_dweb_backend::backend::Backend::from_dependencies(
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
    async fn test_replicate_group() -> Result<()> {
        // Initialize the app
        let path = TmpDir::new("test_api_repo_file_operations").await?;

        let store2 = iroh_blobs::store::fs::Store::load(path.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path.to_path_buf().join("test2").as_path(),
            "test2".to_string(),
        )
        .await?;
        let backend2 = save_dweb_backend::backend::Backend::from_dependencies(
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
    async fn test_refresh_endpoint() -> Result<()> {
        // Initialize the app
        let path = TmpDir::new("test_api_repo_file_operations").await?;

        // Initialize backend2 first
        let store2 = iroh_blobs::store::fs::Store::load(path.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path.to_path_buf().join("test2").as_path(),
            "test2".to_string(),
        )
        .await?;
        let backend2 = save_dweb_backend::backend::Backend::from_dependencies(
            &path.to_path_buf(),
            veilid_api2.clone(),
            update_rx2,
            store2,
        )
        .await
        .unwrap();

        // Initialize the main backend after backend2
        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
        }

        // Create group and repo in backend2
        let mut group = backend2.create_group().await?;
        let join_url = group.get_url();
        group.set_name(TEST_GROUP_NAME).await?;

        let repo = group.create_repo().await?;
        repo.set_name(TEST_GROUP_NAME).await?;

        // Upload a file
        let file_name = "example.txt";
        let file_content = b"Test content for file upload";
        repo.upload(&file_name, file_content.to_vec()).await?;

        // Give the upload some time to propagate through the network
        // Simple polling function with backoff
        async fn poll_until_success(
            attempts: u32, 
            base_delay_ms: u64, 
            max_delay_ms: u64, 
            check_condition: impl Fn() -> Result<bool>
        ) -> Result<()> {
            let mut delay_ms = base_delay_ms;
            for attempt in 0..attempts {
                if check_condition()? {
                    return Ok(());
                }
                
                if attempt < attempts - 1 {
                    let jitter = (attempt as u64 * 37) % (delay_ms / 4);
                    let sleep_duration = std::cmp::min(
                        delay_ms + jitter, 
                        max_delay_ms
                    );
                    
                    tokio::time::sleep(Duration::from_millis(sleep_duration)).await;
                    delay_ms = std::cmp::min(delay_ms * 2, max_delay_ms);
                }
            }
            
            anyhow::bail!("Operation timed out after {} attempts", attempts)
        }

        // Initialize the app
        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Join the group from the first backend and wait for it to be ready
        {
            let backend = get_backend().await?;
            backend.join_from_url(join_url.as_str()).await?;
            
            // Using a simpler approach with manual retries
            let group_id = group.id().clone();
            let mut found = false;
            
            for attempt in 0..10 {
                match backend.list_groups().await {
                    Ok(groups) => {
                        if groups.iter().any(|g| g.id() == group_id) {
                            found = true;
                            break;
                        }
                        eprintln!("Group not found yet (attempt {}), waiting...", attempt + 1);
                    }
                    Err(e) => {
                        eprintln!("Error checking groups (attempt {}): {}", attempt + 1, e);
                    }
                }
                
                let delay = 500 * (1 << attempt.min(5)); // Exponential backoff, max 16s
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            
            if !found {
                anyhow::bail!("Failed to verify group join completion after multiple attempts");
            }
        }

        // Call refresh endpoint and handle response
        let group_id_str = group.id().to_string();
        let mut refresh_data = serde_json::Value::Null;
        
        // Make multiple attempts for the refresh operation
        for attempt in 0..5 {
            // Create a fresh request each time since they can't be reused
            let refresh_req = test::TestRequest::post()
                .uri(&format!("/api/groups/{}/refresh", group_id_str))
                .to_request();
                
            let refresh_resp = test::call_service(&app, refresh_req).await;
            if !refresh_resp.status().is_success() {
                eprintln!("Refresh failed with status: {} (attempt {})", refresh_resp.status(), attempt + 1);
                tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1))).await;
                continue;
            }
            
            // Try to parse the response body
            let body = test::read_body(refresh_resp).await;
            match serde_json::from_slice::<serde_json::Value>(&body) {
                Ok(data) => {
                    refresh_data = data;
                    if refresh_data["status"] == "success" {
                        if let Some(refreshed_files) = refresh_data["refreshed_files"].as_array() {
                            if refreshed_files.iter().any(|f| f.as_str() == Some(file_name)) {
                                break; // Success case
                            }
                        }
                    }
                    eprintln!("Refresh response not as expected (attempt {}): {:?}", attempt + 1, refresh_data);
                }
                Err(e) => {
                    eprintln!("Error parsing refresh response (attempt {}): {}", attempt + 1, e);
                }
            }
            
            // Wait before retrying
            tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1))).await;
        }
        
        // Check if we got a successful response from any attempt
        assert_eq!(refresh_data["status"], "success", "Refresh should return success after multiple attempts");
        
        // Verify file is accessible with retry logic
        let repo_id_str = repo.id().to_string();
        let mut got_file_data = Vec::new();
        let file_name_clone = file_name.to_string();
        
        // Manual retry approach without capturing app
        for attempt in 0..10 {
            // Create a fresh request for each attempt
            let get_file_req = test::TestRequest::get()
                .uri(&format!(
                    "/api/groups/{}/repos/{}/media/{}",
                    group_id_str, repo_id_str, file_name_clone
                ))
                .to_request();
                
            let get_file_resp = test::call_service(&app, get_file_req).await;
            if !get_file_resp.status().is_success() {
                eprintln!("File not yet available, status: {} (attempt {})", 
                        get_file_resp.status(), attempt + 1);
                tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1))).await;
                continue;
            }
            
            got_file_data = test::read_body(get_file_resp).await.to_vec();
            if got_file_data.as_slice() == file_content {
                break; // Success - exit the retry loop
            } else {
                eprintln!("File content mismatch (attempt {}), retrying...", attempt + 1);
                tokio::time::sleep(Duration::from_millis(1000 * (attempt + 1))).await;
            }
        }
        
        // Final verification
        assert_eq!(
            got_file_data.as_slice(),
            file_content,
            "Downloaded file content should match"
        );

        // Verify the refresh response had the expected format
        let refreshed_files = refresh_data["refreshed_files"]
            .as_array()
            .expect("refreshed_files should be an array");
            
        assert!(
            refreshed_files.iter().any(|f| f.as_str() == Some(file_name)),
            "File should be in refreshed_files list"
        );

        // Clean up in reverse order of initialization - with grace periods
        backend2.stop().await?;
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }
        
        // Allow time for clean shutdown
        tokio::time::sleep(Duration::from_secs(1)).await;
        veilid_api2.shutdown().await;

        Ok(())
    }

    #[actix_web::test]
    async fn test_repo_permissions() -> Result<()> {
        // Initialize the app
        let path = TmpDir::new("test_repo_permissions").await?;

        // Initialize backend2 first (this will be the creator of the group/repo)
        let store2 = iroh_blobs::store::fs::Store::load(path.to_path_buf().join("iroh2")).await?;
        let (veilid_api2, update_rx2) = save_dweb_backend::common::init_veilid(
            path.to_path_buf().join("test2").as_path(),
            "test2".to_string(),
        )
        .await?;
        let backend2 = save_dweb_backend::backend::Backend::from_dependencies(
            &path.to_path_buf(),
            veilid_api2.clone(),
            update_rx2,
            store2,
        )
        .await
        .unwrap();

        // Initialize the main backend (this will join the group)
        BACKEND.get_or_init(|| init_backend(path.to_path_buf().as_path()));
        {
            let backend = get_backend().await?;
            backend.start().await.expect("Backend failed to start");
        }

        // Create group and repo in backend2 (creator)
        let mut group = backend2.create_group().await?;
        let join_url = group.get_url();
        group.set_name(TEST_GROUP_NAME).await?;

        let repo = group.create_repo().await?;
        repo.set_name(TEST_GROUP_NAME).await?;

        // Verify creator has write access
        let creator_repo: SnowbirdRepo = repo.clone().into();
        assert!(creator_repo.can_write, "Creator should have write access");

        // Join the group with the main backend
        {
            let backend = get_backend().await?;
            backend.join_from_url(join_url.as_str()).await?;
        }

        let app = test::init_service(
            App::new()
                .service(status)
                .service(web::scope("/api").service(groups::scope())),
        )
        .await;

        // Get the repo info through the API for the joined backend
        let get_repo_req = test::TestRequest::get()
            .uri(&format!(
                "/api/groups/{}/repos/{}",
                group.id().to_string(),
                repo.id().to_string()
            ))
            .to_request();
        let joined_repo: SnowbirdRepo = test::call_and_read_body_json(&app, get_repo_req).await;

        // Verify joined backend has read-only access
        assert!(!joined_repo.can_write, "Joined backend should have read-only access");

        // List repos to verify permissions are consistent
        let list_repos_req = test::TestRequest::get()
            .uri(&format!("/api/groups/{}/repos", group.id().to_string()))
            .to_request();
        let repos_response: ReposResponse = test::call_and_read_body_json(&app, list_repos_req).await;
        
        assert_eq!(repos_response.repos.len(), 1, "Should see one repo");
        assert!(!repos_response.repos[0].can_write, "Listed repo should show read-only access");

        // Clean up
        backend2.stop().await?;
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        veilid_api2.shutdown().await;

        Ok(())
    }
}
