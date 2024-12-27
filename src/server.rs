use actix_web::{HttpServer, web};
use std::net::SocketAddr;
use actix_web::App;
use std::thread;
use actix_cors::Cors;
use crate::config::Config;
use crate::route::{eligible::get_eligible,account::bind_sol_address};
use crate::route::account::{get_account, get_account_invitees, get_account_rebate, get_mint_records,get_account_invitees_count};
use crate::route::stat::{get_mint_progress, get_total_commission};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: rbatis::RBatis,
}

pub async fn run_server(app_state: AppState) {
    thread::Builder::new()
        .spawn(move || {
            actix_rt::System::new().block_on(async move {
                run_rpc_server(app_state).await
            });
        })
        .expect("failed to start endpoint server");

}

pub async fn run_rpc_server(app_state: AppState) {
    let works_number = app_state.config.workers;
    let bind_to = SocketAddr::new("0.0.0.0".parse().unwrap(),
                                  app_state.config.port as u16);
    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .app_data(web::Data::new(app_state.clone()))
            .route("/get_eligible", web::get().to(get_eligible))
            .route("/get_account", web::get().to(get_account))
            .route("/bind_sol_address", web::post().to(bind_sol_address))
            .route("/get_mint_records", web::get().to(get_mint_records))
            .route("/get_account_invitees", web::get().to(get_account_invitees))
            .route("/get_account_invitees_count", web::get().to(get_account_invitees_count))
            .route("/get_account_rebate", web::get().to(get_account_rebate))
            .route("/get_mint_progress", web::get().to(get_mint_progress))
            .route("/get_total_commission", web::get().to(get_total_commission))
    })
        .workers(works_number as usize)
        .bind(&bind_to)
        .expect("failed to bind")
        .run()
        .await
        .expect("failed to run endpoint server");
}