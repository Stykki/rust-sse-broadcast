use std::{io, sync::Arc, time::Duration};

use ::sysinfo::{CpuExt, System, SystemExt};
use actix_web::{
    get,
    middleware::Logger,
    post,
    web::{self, Json},
    App, HttpResponse, HttpServer, Responder,
};
use actix_web_lab::{extract::Path, respond::Html};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::time::interval;

use crate::broadcast::BroadcastStore;

mod broadcast;

struct Sys {
    inner: Mutex<System>,
    is_sending_cpu_usage: Mutex<bool>,
}

impl Sys {
    fn new() -> Arc<Self> {
        let this = Arc::new(Sys {
            inner: Mutex::new(System::new()),
            is_sending_cpu_usage: Mutex::new(false),
        });
        this
    }

    fn get_cpu_usage(&self) -> Vec<f32> {
        let mut sys = self.inner.lock();
        sys.refresh_cpu();
        sys.cpus()
            .iter()
            .map(|cpu| cpu.cpu_usage())
            .collect::<Vec<_>>()
    }
}

impl BroadcastStore {
    fn send_cpu_usage(this: Arc<Self>, sys: Arc<Sys>) {
        if *sys.is_sending_cpu_usage.lock() {
            return;
        }
        *sys.is_sending_cpu_usage.lock() = true;

        actix_web::rt::spawn(async move {
            let mut interval = interval(Duration::from_secs(2));

            loop {
                interval.tick().await;

                if !this.has_cpu_listener() {
                    *sys.is_sending_cpu_usage.lock() = false;
                    break;
                }

                let cpu_usage = sys.get_cpu_usage();
                let cpu_usage = cpu_usage
                    .iter()
                    .map(|cpu| format!("{}%", cpu.to_string()))
                    .collect::<Vec<_>>()
                    .join("\n");

                this.broadcast("cpu", &cpu_usage).await;
            }
        });
    }

    fn has_cpu_listener(&self) -> bool {
        self.client_count("cpu") > 0
    }
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let broadcast_store = broadcast::BroadcastStore::new();

    let sys = Sys::new();

    log::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::from(Arc::clone(&broadcast_store)))
            .app_data(web::Data::from(Arc::clone(&sys)))
            .service(index)
            .service(event_stream)
            .service(broadcast_msg)
            .service(sysinfo)
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
    system: web::Data<Sys>,
    Path((channel,)): Path<(String,)>,
) -> impl Responder {
    if channel == "cpu" {
        BroadcastStore::send_cpu_usage(Arc::clone(&broadcast_store), Arc::clone(&system));
    }

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

#[get("/sysinfo")]
async fn sysinfo(system: web::Data<Sys>) -> impl Responder {
    Json(system.get_cpu_usage())
}
