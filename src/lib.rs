#![recursion_limit = "256"]

#[cfg(target_os = "android")]
pub mod android_bridge;
#[cfg(target_os = "android")]
pub mod jni_globals;

#[cfg(target_os = "android")]
pub mod status_updater;

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
    use crate::media::{download_file, upload_file, scope}; 
    use crate::models::GroupRepoPath;

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

        // Add a short sleep to ensure the async repo creation has completed
        tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;

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
async fn test_upload_file() -> Result<()> {
    let mut app = test::init_service(
        App::new().service(
            scope()
        )
    ).await;

    let group_repo_path = GroupRepoPath {
        group_id: "test_group".to_string(),
        repo_id: "test_repo".to_string(),
    };

    let payload = web::Bytes::from_static(b"test file content");
    let req = test::TestRequest::post()
        .uri("/media")
        .set_payload(payload)
        .to_request();

    let resp = test::call_service(&mut app, req).await;
    assert!(resp.status().is_success());

    Ok(())
}

#[actix_web::test]
async fn test_download_file() -> Result<()> {
    let mut app = test::init_service(
        App::new().service(
            scope()
        )
    ).await;

    let group_repo_path = GroupRepoPath {
        group_id: "test_group".to_string(),
        repo_id: "test_repo".to_string(),
    };

    let req = test::TestRequest::get()
        .uri("/media")
        .to_request();

    let resp = test::call_service(&mut app, req).await;
    assert!(resp.status().is_success());

    Ok(())
}
}
