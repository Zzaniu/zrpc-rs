use crate::common::ServiceInstance;
use crate::register::Register;
use tonic::body::BoxBody;
use tonic::codegen::http::{Request, Response};
use tonic::codegen::Service;
use tonic::service::Routes;
use tonic::transport::server::Router;
use tool::log::trace_log::info;
use tower::layer::util::Identity;
use tower::Layer;

pub struct Server<R> {
    register: R,
    server_instance: ServiceInstance,
}

impl<R> Server<R>
where
    R: Register + Send + 'static,
{
    pub fn new(register: R, server_instance: ServiceInstance) -> Self {
        Self {
            register,
            server_instance,
        }
    }

    pub async fn serve<L, F>(self, f: F)
    where
        F: Fn(tonic::transport::Server<Identity>) -> Router<L> + Send + 'static,
        L: Layer<Routes>,
        L::Service:
            Service<Request<BoxBody>, Response = Response<BoxBody>> + Clone + Send + 'static,
        <<L as Layer<Routes>>::Service as Service<Request<BoxBody>>>::Future: Send + 'static,
        <<L as Layer<Routes>>::Service as Service<Request<BoxBody>>>::Error:
            Into<Box<dyn std::error::Error + Send + Sync>> + Send,
    {
        let Self {
            register,
            server_instance,
        } = self;
        let addr = server_instance.endpoint.parse().unwrap();

        let router = f(tonic::transport::Server::builder());
        tokio::spawn(async move {
            register.register(&server_instance).await.unwrap();
        });
        info!("Server listening on: {}", addr);
        router
            .serve_with_shutdown(addr, Self::wait_for_quit())
            .await
            .unwrap();
    }

    async fn wait_for_quit() {
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.unwrap();
        }
        #[cfg(unix)]
        {
            use tokio::signal::unix::SignalKind;
            // SIGINT, ctrl_c/kill -2
            let mut signal_ctrl_c = tokio::signal::unix::signal(SignalKind::interrupt())
                .expect("Failed to catch the SIGINT signal");
            // SIGTERM, kill
            let mut signal_term = tokio::signal::unix::signal(SignalKind::terminate())
                .expect("Failed to catch the SIGTERM signal");
            tokio::select! {
                _ = signal_ctrl_c.recv() => info!("Received SIGTERM signal"),
                _ = signal_term.recv() => info!("Received SIGINT signal"),
            }
        }
    }
}
