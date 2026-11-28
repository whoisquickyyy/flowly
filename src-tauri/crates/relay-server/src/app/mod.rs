pub mod forwarded;
pub mod health;
pub mod metrics;
pub mod router;
pub mod shutdown;
pub mod state;

pub use state::{AppState, Shared};

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;

use crate::config::Config;
use crate::rooms::sweeper;

pub struct Server {
    listener: TcpListener,
    router: Router,
    address: SocketAddr,
    state: Shared,
}

impl Server {
    
    
    
    pub async fn bind(config: Config) -> io::Result<Self> {
        let listener =
            TcpListener::bind((config.server.address.as_str(), config.server.port)).await?;

        let address = listener.local_addr()?;
        let state = AppState::shared(config);
        let router = router::build(Arc::clone(&state));

        Ok(Self {
            listener,
            router,
            address,
            state,
        })
    }

    
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    
    pub fn state(&self) -> Shared {
        Arc::clone(&self.state)
    }

    
    pub async fn serve(self, shutdown: impl Future<Output = ()> + Send + 'static) -> io::Result<()> {
        let sweeper = sweeper::spawn(Arc::clone(&self.state));
        let state = Arc::clone(&self.state);

        let signal = async move {
            shutdown.await;
            state.begin_shutdown();
        };

        let result = axum::serve(
            self.listener,
            self.router
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(signal)
        .await;

        sweeper.abort();
        result
    }
}
