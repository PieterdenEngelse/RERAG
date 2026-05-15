use crate::monitoring::metrics::REQUEST_LATENCY_MS;
use crate::monitoring::{clear_trace_id, record_http_request, set_trace_id};
use actix_service::{Service, Transform};
use actix_web::{
    dev::{ServiceRequest, ServiceResponse},
    http::header,
    Error,
};
use opentelemetry::{
    global,
    trace::{Span, Status, Tracer},
};
use std::future::{ready, Ready};
use std::task::{Context, Poll};
use std::time::Instant;
use tracing::{debug_span, Instrument};

pub struct TraceMiddleware {
    server: &'static str,
}

impl Default for TraceMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceMiddleware {
    pub fn new() -> Self {
        Self { server: "search" }
    }

    pub fn new_with_server(server: &'static str) -> Self {
        Self { server }
    }
}

impl<S, B> Transform<S, ServiceRequest> for TraceMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = TraceMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> <Self as Transform<S, ServiceRequest>>::Future {
        ready(Ok(TraceMiddlewareService {
            service,
            server: self.server,
        }))
    }
}

pub struct TraceMiddlewareService<S> {
    service: S,
    server: &'static str,
}

impl<S, B> Service<ServiceRequest> for TraceMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), <Self as Service<ServiceRequest>>::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> <Self as Service<ServiceRequest>>::Future {
        let method = req.method().to_string();
        let route_label = req
            .match_pattern()
            .unwrap_or_else(|| req.path().to_string());
        let user_agent = req
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let request_id = uuid::Uuid::new_v4().to_string();
        let client_ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();
        let server = self.server;

        set_trace_id(request_id.clone());

        let span = debug_span!(
            "http_request",
            method = %method,
            path = %route_label,
            client_ip = %client_ip,
            trace_id = %request_id,
            user_agent = %user_agent,
            server = %server,
        );

        let tracer = global::tracer("ag-backend");
        let span_name = format!("{} {}", method, route_label);
        let mut otel_span = tracer.start(span_name);
        otel_span.set_attribute(opentelemetry::KeyValue::new("http.method", method.clone()));
        otel_span.set_attribute(opentelemetry::KeyValue::new("http.url", route_label.clone()));
        otel_span.set_attribute(opentelemetry::KeyValue::new("http.client_ip", client_ip.clone()));
        otel_span.set_attribute(opentelemetry::KeyValue::new("trace.id", request_id.clone()));
        otel_span.set_attribute(opentelemetry::KeyValue::new("http.user_agent", user_agent.clone()));
        otel_span.set_attribute(opentelemetry::KeyValue::new("server.name", server));

        let start = Instant::now();
        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.instrument(span).await;

            if let Ok(ref response) = res {
                let status = response.status().as_u16();
                let duration_ms = start.elapsed().as_millis() as u64;
                let status_class = format!("{}xx", status / 100);

                if status >= 400 {
                    otel_span.set_status(Status::Error {
                        description: format!("HTTP {}", status).into(),
                    });
                } else {
                    otel_span.set_status(Status::Ok);
                }

                otel_span.set_attribute(opentelemetry::KeyValue::new(
                    "http.status_code",
                    status as i64,
                ));
                otel_span.set_attribute(opentelemetry::KeyValue::new(
                    "http.duration_ms",
                    duration_ms as i64,
                ));

                let duration_ms_f64 = duration_ms as f64;
                REQUEST_LATENCY_MS
                    .with_label_values(&[server, &method, &route_label, &status_class])
                    .observe(duration_ms_f64);

                let is_error = status >= 500;
                record_http_request(duration_ms_f64, is_error, &status_class, server);

                if status >= 400 || duration_ms >= 500 {
                    tracing::info!(
                        method = %method,
                        path = %route_label,
                        status = status,
                        duration_ms = duration_ms,
                        trace_id = %request_id,
                        server = %server,
                        "request completed"
                    );
                } else {
                    tracing::debug!(
                        method = %method,
                        path = %route_label,
                        status = status,
                        duration_ms = duration_ms,
                        trace_id = %request_id,
                        server = %server,
                        "request completed"
                    );
                }
            } else {
                otel_span.set_status(Status::Error {
                    description: "Request failed".into(),
                });
            }

            clear_trace_id();
            res
        })
    }
}
