#![recursion_limit = "256"]

#[cfg(target_os = "android")]
pub mod android_bridge;
#[cfg(target_os = "android")]
pub mod jni_globals;
#[cfg(target_os = "android")]
pub mod status_updater;

#[cfg(target_os = "android")]
pub mod jni_globals;

#[cfg(target_os = "macos")]
pub mod mac;

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
    use super::*;
    use actix_web::{test, web, App};
    use anyhow::Result;
    use models::{RequestName, SnowbirdGroup, SnowbirdRepo};
    use serde::{Deserialize, Serialize};
    use server::server::{get_backend, init_backend, status, BACKEND};
    use tmpdir::TmpDir;
    use serde_json::json;

    #[derive(Serialize, Deserialize)]
    struct GroupsResponse {
        groups: Vec<SnowbirdGroup>,
    }

    #[derive(Serialize, Deserialize)]
    struct ReposResponse {
        repos: Vec<SnowbirdRepo>,
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
    async fn test_api_file_operations() -> Result<()> {
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
        let create_group_resp: serde_json::Value = test::call_and_read_body_json(&app, create_group_req).await;
        let group_id = create_group_resp["key"].as_str().expect("No group key returned");

        // Step 2: Create a repo via the API
        let create_repo_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/repos", group_id))
            .set_json(json!({ "name": "Test Repo" }))
            .to_request();
        let create_repo_resp: serde_json::Value = test::call_and_read_body_json(&app, create_repo_req).await;

        let repo_id = create_repo_resp["key"].as_str().expect("No repo key returned");

        // Step 3: Upload a file to the repository
        let file_name = "example.txt";
        let file_content = b"Test content for file upload";

        let upload_req = test::TestRequest::post()
            .uri(&format!("/api/groups/{}/repos/{}/media?file_name={}", group_id, repo_id, file_name))
            .set_payload(file_content.to_vec())
            .to_request();
        let upload_resp = test::call_service(&app, upload_req).await;

        // Check upload success
        assert!(upload_resp.status().is_success(), "File upload failed");

        // Step 4: List files in the repository
        let list_files_req = test::TestRequest::get()
            .uri(&format!("/api/groups/{}/repos/{}/media", group_id, repo_id))
            .to_request();
        let list_files_resp: serde_json::Value = test::call_and_read_body_json(&app, list_files_req).await;

        println!("List files response: {:?}", list_files_resp);

        // Now check if the response is an array directly
        if let Some(files_array) = list_files_resp.as_array() {
            assert_eq!(
                files_array.len(),
                1,
                "There should be one file in the repo"
            );

            // Ensure the file name matches what we uploaded
            let file_obj = &files_array[0];
            assert_eq!(
                file_obj["name"].as_str().unwrap(),
                "example.txt",
                "The listed file should match the uploaded file"
            );

            let file_name = file_obj["name"].as_str().expect("File name not found");
            assert_eq!(file_name, "example.txt", "File name does not match");

            // Step 5: Now, test the file download response
            let download_file_req = test::TestRequest::get()
                .uri(&format!("/api/groups/{}/repos/{}/media?file_name={}", group_id, repo_id, "example.txt"))
                .to_request();

            let download_file_resp = test::call_service(&app, download_file_req).await;
            let download_file_status = download_file_resp.status();
            let download_file_body = test::read_body(download_file_resp).await;

            println!("Download file response status: {:?}", download_file_status);
            println!("Download file response body: {:?}", download_file_body);

            // Verify the response status
            assert_eq!(
                download_file_status, 200,
                "Expected successful file download, but got status: {:?}", download_file_status
            );

            // Compare the downloaded raw file content to the original content
            assert_eq!(
                &download_file_body[..],  // Ensure byte-by-byte comparison
                b"Test content for file upload",
                "Downloaded file content does not match uploaded content"
            );

            // Step 6: Delete the file from the repository
            let delete_file_req = test::TestRequest::delete()
                .uri(&format!("/api/groups/{}/repos/{}/media?file_name={}", group_id, repo_id, "example.txt"))
                .to_request();
            let delete_resp = test::call_service(&app, delete_file_req).await;

            assert!(delete_resp.status().is_success(), "File deletion failed");

            // Step 7: Verify the file is no longer listed
            let list_files_after_deletion_req = test::TestRequest::get()
                .uri(&format!("/api/groups/{}/repos/{}/media", group_id, repo_id))
                .to_request();
            let list_files_after_deletion_resp: serde_json::Value = test::call_and_read_body_json(&app, list_files_after_deletion_req).await;

            assert!(
                list_files_after_deletion_resp.as_array().unwrap().is_empty(),
                "File list should be empty after file deletion"
            );
        } else {
            panic!("The response is not an array as expected");
        }

        // Clean up: Stop the backend
        {
            let backend = get_backend().await?;
            backend.stop().await.expect("Backend failed to stop");
        }

        Ok(())
    }

}
