    use actix_web::{web, get, post, Responder, HttpResponse};
    use serde_json::json;
    use crate::models::{RequestName, SnowbirdGroup};
    use crate::error::AppResult;
    use crate::logging::android_log;
    use crate::log_debug;
    use crate::constants::TAG;
    use crate::server::server::get_backend;
    use crate::utils::create_veilid_typedkey_from_base64;
    use crate::models::IntoSnowbirdGroupsWithNames;
    use save_dweb_backend::common::DHTEntity;

    pub fn scope() -> actix_web::Scope {
        web::scope("/groups")
            .service(get_groups)
            .service(get_group)
            .service(create_group)
    }

    #[get("")]
    async fn get_groups() -> AppResult<impl Responder> {
        let backend = get_backend().await?;
        let groups = backend.list_groups().await.unwrap();
        let snowbird_groups = groups.into_snowbird_groups_with_names().await;
    
        Ok(HttpResponse::Ok().json(json!({ "groups": snowbird_groups })))
    }

    #[get("/{group_id}")]
    async fn get_group(group_id: web::Path<String>) -> AppResult<impl Responder> {
        let mut backend = get_backend().await?;
        log_debug!(TAG, "got backend");

        let group_id = group_id.into_inner();
        // let key_string = "nN7W0-JiuhIcCWhy4Sw0J7mfDWWE9OtnCfAbLmwLbq0";
        let key = create_veilid_typedkey_from_base64(group_id.as_str()).unwrap();
        log_debug!(TAG, "got key {}", key);

        let backend_group = backend.get_group(key).await?;
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

        let mut backend = get_backend().await?;
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

    // Later
    // #[patch("/{group_id}")]
    // async fn update_group(path: web::Path<String>) -> AppResult<impl Responder> {
    //     let backend = get_backend().await?;

    //     let group_id = path.into_inner();

    //     // let group = backend.get_group(con).await?;

    //     // group.set_name("foo").await.expect(dweb::UNABLE_TO_SET_GROUP_NAME);

    //     Ok(HttpResponse::Ok().json(json!({
    //         "name": "My Group"
    //     })))
    // }