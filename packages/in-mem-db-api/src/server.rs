//! Hyper server setup and request handling.

use std::net::SocketAddr;
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::{Bytes, Incoming as IncomingBody};
use hyper::{Request, Response, Result as HyperResult};
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder as ConnectionBuilder;
use tokio::net::TcpListener;

use crate::router::Router;

/// HTTP server for the in-memory database API.
pub struct Server {
    addr: SocketAddr,
    router: Arc<Router>,
}

impl Server {
    /// Creates a new server instance.
    ///
    /// # Arguments
    /// * `addr` - Socket address to bind to
    /// * `router` - Request router
    pub fn new(addr: SocketAddr, router: Router) -> Self {
        Self {
            addr,
            router: Arc::new(router),
        }
    }

    /// Starts the HTTP server.
    ///
    /// # Returns
    /// `Result<(), std::io::Error>` indicating success or failure.
    pub async fn serve(self) -> Result<(), std::io::Error> {
        let listener = TcpListener::bind(self.addr).await?;
        println!("Server listening on http://{}", self.addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let router = Arc::clone(&self.router);

            tokio::task::spawn(async move {
                let builder = ConnectionBuilder::new(TokioExecutor::new());
                if let Err(err) = builder
                    .serve_connection(
                        io,
                        hyper::service::service_fn(move |req| handle_request(req, router.clone())),
                    )
                    .await
                {
                    eprintln!("Error serving connection: {}", err);
                }
            });
        }
    }
}

/// Handles an incoming HTTP request.
async fn handle_request(
    req: Request<IncomingBody>,
    router: Arc<Router>,
) -> HyperResult<Response<Full<Bytes>>> {
    match router.route(req).await {
        Ok(response) => Ok(response.map(Full::new)),
        Err(err) => {
            eprintln!("Error handling request: {}", err);
            let response = Response::builder()
                .status(500)
                .body(Full::new(Bytes::from("Internal Server Error")))
                .unwrap();
            Ok(response)
        }
    }
}
