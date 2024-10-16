use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use futures::future::{ok, Ready};
use std::sync::Arc;

pub struct RouteDumper {
    logger: Arc<dyn Fn(&str) + Send + Sync>,
}

impl RouteDumper {
    pub fn new<F>(logger: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        RouteDumper {
            logger: Arc::new(logger),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RouteDumper
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RouteDumperMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RouteDumperMiddleware {
            service,
            logger: self.logger.clone(),
        })
    }
}

pub struct RouteDumperMiddleware<S> {
    service: S,
    logger: Arc<dyn Fn(&str) + Send + Sync>,
}

impl<S, B> Service<ServiceRequest> for RouteDumperMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = S::Future;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let route_info = format!("{} {}", req.method(), req.path());
        (self.logger)(&route_info);
        self.service.call(req)
    }
}