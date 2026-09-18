use fluida_server::{app, config::Config};

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    let address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("failed to bind server");
    println!("FluidaOS ready at http://{address}");
    axum::serve(
        listener,
        app(config)
            .await
            .into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .expect("server failed");
}
