use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderValue, RETRY_AFTER},
    Error, HttpResponse,
};
use futures_util::future::LocalBoxFuture;
use std::sync::Arc;
use tokio::sync::{Semaphore, TryAcquireError};

pub struct UploadConcurrencyMiddleware {
    semaphore: Arc<Semaphore>,
}

impl UploadConcurrencyMiddleware {
    pub fn new(semaphore: Arc<Semaphore>) -> Self {
        Self { semaphore }
    }
}

impl<S, B> Transform<S, ServiceRequest> for UploadConcurrencyMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = UploadConcurrencyService<S>;
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(UploadConcurrencyService {
            service: Arc::new(service),
            semaphore: Arc::clone(&self.semaphore),
        }))
    }
}

pub struct UploadConcurrencyService<S> {
    service: Arc<S>,
    semaphore: Arc<Semaphore>,
}

impl<S, B> Service<ServiceRequest> for UploadConcurrencyService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Monitoring paths bypass the concurrency limit
        if req.path().starts_with("/monitoring") {
            let service = Arc::clone(&self.service);
            return Box::pin(async move {
                let res = service.call(req).await?;
                Ok(res.map_into_left_body())
            });
        }

        let semaphore = Arc::clone(&self.semaphore);
        let service = Arc::clone(&self.service);

        Box::pin(async move {
            match semaphore.clone().try_acquire_owned() {
                Ok(permit) => {
                    let res = service.call(req).await?;
                    drop(permit);
                    Ok(res.map_into_left_body())
                }
                Err(TryAcquireError::NoPermits) => {
                    tracing::warn!(
                        available_permits = 0,
                        "Upload concurrency limit reached — returning 503"
                    );
                    let resp = HttpResponse::ServiceUnavailable()
                        .insert_header((
                            RETRY_AFTER,
                            HeaderValue::from_static("1"),
                        ))
                        .json(serde_json::json!({
                            "status": "overloaded",
                            "message": "Upload server at capacity — retry after 1s",
                        }));
                    Ok(req.into_response(resp.map_into_right_body()))
                }
                Err(TryAcquireError::Closed) => {
                    let resp = HttpResponse::ServiceUnavailable()
                        .json(serde_json::json!({
                            "status": "unavailable",
                            "message": "Upload server shutting down",
                        }));
                    Ok(req.into_response(resp.map_into_right_body()))
                }
            }
        })
    }
}
