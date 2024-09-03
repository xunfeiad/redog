use actix_web::{middleware, middleware::Logger, web, App, HttpServer};

use api::{route::config, websocket::};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::time::Duration;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

pub struct AppState {
    pub app_name: String,
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::INFO)
        // completes the builder.
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    // load TLS keys
    // to create a self-signed temporary cert for testing:
    // `openssl req -x509 -newkey rsa:4096 -nodes -keyout key.pem -out cert.pem -days 365 -subj '/CN=localhost'`

    // let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    // builder
    //     .set_private_key_file("key.pem", SslFiletype::PEM)
    //     .unwrap();
    // builder.set_certificate_chain_file("cert.pem").unwrap();

    let (chat_server, server_tx) = ChatServer::new();
    HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(AppState {
                app_name: "Actx_Web".to_string(),
            }))
            .wrap(middleware::DefaultHeaders::new().add(("APP-NAME", "Actx-Web")))
            .wrap(Logger::default())
            .configure(config)
    })
    .keep_alive(Duration::from_secs(100))
    // .bind_openssl("0.0.0.0:8080", builder)?
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
