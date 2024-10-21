use actix_web::{web, delete, get, post, Responder, HttpResponse};
use save_dweb_backend::common::DHTEntity;
use serde_json::json;
use crate::error::AppResult;
use crate::log_debug;
use crate::models::IntoSnowbirdGroupsWithNames;
use crate::models::{RequestName, SnowbirdGroup, RequestUrl};
use crate::repos;
use crate::constants::TAG;
use crate::server::server::get_backend;
use crate::utils::create_veilid_cryptokey_from_base64;

pub fn scope() -> actix_web::Scope {
    web::scope("/groups")
        .service(get_groups)
        .service(create_group)
        .service(join_group_from_url)
        .service(
            web::scope("/{group_id}")
                .service(delete_group)
                .service(get_group)
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
