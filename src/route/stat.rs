use std::cmp::min;
use std::fmt::format;
use std::ops::{Div, Mul};
use std::str::FromStr;
use actix_web::{HttpRequest, HttpResponse, web};
use bigdecimal::BigDecimal;
use num::BigUint;
use crate::db;
use crate::route::BackendResponse;
use crate::route::err::BackendError;
use crate::server::AppState;

pub async fn get_mint_progress(data: web::Data<AppState>, _req: HttpRequest)
                          -> actix_web::Result<HttpResponse> {
    match db::get_total_mint(&data.db).await {
        Ok(total_amount) => {
            let launch_max_amount = BigDecimal::from(data.config.launch_max_amount);
            let total_amount = BigDecimal::from_str(&total_amount.to_string()).unwrap();
            let progress = min(total_amount.div(launch_max_amount),BigDecimal::from(1));
            let progress = progress.mul(BigDecimal::from(100));
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(format!("{:.2}",progress))
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_total_mint failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get_mint_progress failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}
pub async fn get_total_commission(data: web::Data<AppState>, _req: HttpRequest)
                               -> actix_web::Result<HttpResponse> {
    match db::get_total_rebate(&data.db).await {
        Ok(total_amount) => {
            let total_rebate = BigDecimal::from_str(&total_amount.to_string())
                .unwrap().div(BigDecimal::from(10));
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(total_rebate.to_string())
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_total_commission failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get_total_commission failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}