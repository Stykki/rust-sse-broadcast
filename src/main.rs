use std::{io, sync::Arc};

use actix_web::{get, middleware::Logger, post, web, App, HttpResponse, HttpServer, Responder};
use actix_web_lab::{extract::Path, respond::Html};
use serde::{Deserialize, Serialize};

use crate::broadcast::BroadcastStore;

mod broadcast;

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let broadcast_store = broadcast::BroadcastStore::new();

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::from(Arc::clone(&broadcast_store)))
            .service(index)
            .service(event_stream)
            .service(broadcast_msg)
            .service(bc)
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 8080))?
    .workers(2)
    .run()
    .await
}

#[get("/")]
async fn index() -> impl Responder {
    Html(include_str!("index.html").to_string())
}

#[get("/events/{channel}")]
async fn event_stream(
    broadcast_store: web::Data<BroadcastStore>,
    Path((channel,)): Path<(String,)>,
) -> impl Responder {
    broadcast_store.add_client(channel).await
}

#[post("/broadcast/{channel}/{msg}")]
async fn broadcast_msg(
    broadcast_store: web::Data<BroadcastStore>,
    Path((channel, msg)): Path<(String, String)>,
) -> impl Responder {
    broadcast_store.broadcast(&channel, &msg).await;
    HttpResponse::Ok().body("msg sent")
}

#[derive(Serialize, Deserialize)]
struct BcPayload {
    channel: String,
    msg: String,
}

#[post("/broadcast/")]
async fn bc(
    bc_payload: web::Json<BcPayload>,
    broadcast_store: web::Data<BroadcastStore>,
) -> impl Responder {
    log::info!(
        "broadcasting message '{}' to channel '{}'",
        bc_payload.msg,
        bc_payload.channel
    );

    broadcast_store
        .broadcast(&bc_payload.channel, &bc_payload.msg)
        .await;

    return HttpResponse::Ok().body("msg sent");
}
