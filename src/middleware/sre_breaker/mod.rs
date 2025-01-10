use dashmap::DashMap;
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tonic::body::BoxBody;
use tonic::codegen::{http, Service};
use tonic::Status;
use tool::sre_breaker::breaker::SreBreaker;

#[derive(Debug, Clone)]
struct ServerSreBreakerGroup(Arc<DashMap<String, Arc<SreBreaker>>>);

impl ServerSreBreakerGroup {
    fn get_sre_breaker(&self, uri_path: String) -> Arc<SreBreaker> {
        let ref_mut = self
            .0
            .entry(uri_path)
            .or_insert_with(|| Arc::new(SreBreaker::default()));
        ref_mut.value().clone()
        // note: 锁在这里释放
    }
}

#[derive(Clone)]
pub struct ServerSreBreakerInner<S> {
    inner: S,
    breaker: ServerSreBreakerGroup,
}

pin_project! {
    pub struct ResponseFuture<F> {
        #[pin]
        inner: F,
        breaker: ServerSreBreakerGroup,
        full_uri_path: String,
    }
}

#[derive(Clone)]
pub struct ServerSreBreaker;

impl<S> tower::Layer<S> for ServerSreBreaker {
    type Service = ServerSreBreakerInner<S>;

    fn layer(&self, service: S) -> Self::Service {
        ServerSreBreakerInner {
            inner: service,
            breaker: ServerSreBreakerGroup(Arc::new(DashMap::new())),
        }
    }
}

impl<S, ReqBody> Service<http::Request<ReqBody>> for ServerSreBreakerInner<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let full_uri_path = req.uri().path().to_owned();
        ResponseFuture {
            breaker: self.breaker.clone(),
            full_uri_path,
            inner: inner.call(req),
        }
    }
}

impl<F, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<http::Response<BoxBody>, E>>,
    E: Send + 'static,
{
    type Output = Result<http::Response<BoxBody>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let sre_breaker = this.breaker.get_sre_breaker(this.full_uri_path.clone());
        if sre_breaker.allow().is_err() {
            let error = Status::unavailable("系统繁忙，请稍后再试");
            return Poll::Ready(Ok(error.into_http()));
        };
        if let Poll::Ready(res) = this.inner.poll(cx) {
            use std::borrow::Borrow;
            if let Ok(res) = res.borrow() {
                let res_status = Status::from_header_map(res.headers());
                if let Some(value) = res_status {
                    // TODO: 目前是只要不是成功的就算失败, 这里需要好好考虑一下
                    if value.code() != tonic::Code::Ok {
                        sre_breaker.mark_failed();
                    } else {
                        sre_breaker.mark_success();
                    }
                } else {
                    sre_breaker.mark_failed();
                }
            } else {
                sre_breaker.mark_failed();
            }
            return Poll::Ready(res);
        }
        Poll::Pending
    }
}
