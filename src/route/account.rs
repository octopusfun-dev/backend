use std::ops::Div;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use actix_web::{HttpRequest, HttpResponse, web};
use bigdecimal::{BigDecimal, ToPrimitive};
use qstring::QString;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use solana_sdk::pubkey::Pubkey;
use crate::db;
use crate::db::tables::{Account, LaunchRecord, QueryAccount};
use crate::route::BackendResponse;
use crate::route::err::BackendError;
use crate::route::utils::get_solana_address_from_parameter;
use crate::server::AppState;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NewAccountReq {
    pub address: String,
    pub referred_by: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BindAccountReq {
    pub address: String,
    pub sol_address: String,
    pub inviter_code: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BindAccountRsp {
    pub invite_code: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AccountRebateRsp {
    pub rebate: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MintRecordsRsp {
    pub page_count: usize,
    pub mint_records: Vec<MintRecordsInfo>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MintRecordsInfo {
    pub address: String,
    pub amount: String,
    pub time: i64,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AccountInvitee {
    pub invitee: String,
    pub mint_amount: String,
    pub rebate: String,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AccountInviteesRsp {
    pub page_count: usize,
    pub invitees: Vec<AccountInvitee>,
}

fn generate_invite_code(seed: [u8;32]) -> String {
    let charset: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut rng = StdRng::from_seed(seed);
    let invite_code: String = (0..6)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect();

    invite_code
}

fn is_valid_invite_code(invite_code: &str) -> bool {
    if invite_code.len() != 6 {
        return false;
    }
    if !invite_code.chars().all(|c| matches!(c, '0'..='9') | matches!(c, 'A'..='Z')) {
        return false;
    }

    true
}

pub async fn bind_sol_address(
    data: web::Data<AppState>,
    msg: web::Json<BindAccountReq>,
) -> actix_web::Result<HttpResponse> {
    let mut rb = data.db.clone();
    if !Pubkey::from_str(&msg.sol_address).is_ok()  {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Invalid solana address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    }

    let mut inviter_account = None;
    if let Some(inviter_code) = msg.inviter_code.clone() {
        if inviter_code != "" {
            if is_valid_invite_code(&inviter_code) {
                let account_by_code = db::get_account_by_inviter_code(&rb, &inviter_code).await.unwrap_or_default();
                if account_by_code.is_none() {
                    let resp = BackendResponse {
                        code: BackendError::InvalidParameters,
                        error: Some("Invite code is not exist".to_owned()),
                        data: None::<()>
                    };
                    return Ok(HttpResponse::Ok().json(resp));
                } else {
                    inviter_account = account_by_code;
                }
            } else {
                let resp = BackendResponse {
                    code: BackendError::InvalidParameters,
                    error: Some("Invite code is invalid".to_owned()),
                    data: None::<()>
                };
                return Ok(HttpResponse::Ok().json(resp));
            }
        }
    }
    let mut address = msg.address.to_lowercase();
    if let Some(ret) = address.strip_prefix("0x") {
        address = ret.to_string();
    };
    let ret =  db::get_query_account_by_address(&rb,address.clone())
        .await;
    if let Err(e) = ret {
        log::error!("get_query_account_by_address failed {:?}",e);
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("get account failed".to_string()),
            data: None::<()>,
        };
        return Ok(HttpResponse::Ok().json(resp));
    }

    let query_account = ret.unwrap();
    if query_account.is_some() && query_account.clone().unwrap().claim_sol_address.is_some() {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Address already bind".to_string()),
            data: None::<()>,
        };
        return Ok(HttpResponse::Ok().json(resp));
    }

    //bind solana address
    let ret = db::get_account_by_address(&rb,&msg.sol_address).await;
    if let Err(e) = ret {
        log::error!("get_account_by_address failed {:?}",e);
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Get account failed".to_string()),
            data: None::<()>,
        };
        return Ok(HttpResponse::Ok().json(resp));
    }

    let sol_account = ret.unwrap();
    let (new_account,inviter,invite_code) = if sol_account.is_none() {
        let pub_key = Pubkey::from_str(&msg.sol_address).unwrap();
        let invite_code = generate_invite_code(pub_key.to_bytes());
        let inviter = inviter_account.map(|x| x.address);
        let point = if inviter.is_some() {
            1000
        } else {
            0
        };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        (Some(Account {
            address: msg.sol_address.clone(),
            invite_code: invite_code.clone(),
            inviter: inviter.clone(),
            create_time: now as i64,
            point,
        }),inviter,Some(invite_code))
    } else {
        (None,None,Some(sol_account.unwrap().invite_code))
    };
    let update_query_account = query_account.map(|_| QueryAccount {
        address: address.clone(),
        claim_sol_address: Some(msg.sol_address.clone()),
        ..Default::default()
    });
    if let Err(e) = db::db_bind_sol_address(&mut rb,update_query_account,
                                            new_account,inviter).await {
        log::error!("db_bind_sol_address failed,{e}");
        let resp = BackendResponse {
            code: BackendError::InternalErr,
            error: Some("Save to db failed".to_string()),
            data: None::<()>,
        };
        return Ok(HttpResponse::Ok().json(resp));
    }

    let bind_account_rsp = BindAccountRsp {
        invite_code,
    };
    let resp = BackendResponse {
        code: BackendError::Ok,
        error: None,
        data: Some(bind_account_rsp) ,
    };
    return Ok(HttpResponse::Ok().json(resp));

}

pub async fn get_account(data: web::Data<AppState>, req: HttpRequest)
                                          -> actix_web::Result<HttpResponse> {
    let query_str = req.query_string();
    let qs = QString::from(query_str);
    let Some(address) = qs.get("address") else {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Not input address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    };

    let Some(address) = get_solana_address_from_parameter(address,&data.db).await else {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Need bind solana address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    };

    match db::get_account_by_address(&data.db,&address).await {
        Ok(Some(account)) => {
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(account)
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Ok(None) => {
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("address not exist".to_owned()),
                data: None::<()>,
            };
            Ok(HttpResponse::Ok().json(resp))
        }
        Err(e) => {
            log::warn!("get_account_by_address failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get account failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}

pub async fn get_account_rebate(data: web::Data<AppState>, req: HttpRequest)
                         -> actix_web::Result<HttpResponse> {
    let query_str = req.query_string();
    let qs = QString::from(query_str);
    let Some(address) = qs.get("address") else {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Not input address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    };

    let Some(address) = get_solana_address_from_parameter(address,&data.db).await else {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Need bind solana address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    };


    match db::get_account_invitees_total_mint(&data.db,&address).await {
        Ok(total_mint) => {
            let total_mint = total_mint.0.to_f64().unwrap_or_default();
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(total_mint * 0.1)
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_account_invitees_total_mint failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get info failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}

pub async fn get_mint_records(data: web::Data<AppState>, req: HttpRequest)
                                -> actix_web::Result<HttpResponse> {
    let query_str = req.query_string();
    let qs = QString::from(query_str);
    let pg_no = qs.get("pg_no").unwrap_or("1").parse::<i32>().unwrap();
    match db::get_launch_records(&data.db,pg_no).await {
        Ok((page_count,records)) => {
            let mint_records = records.iter().map(|r| MintRecordsInfo {
                address: r.address.clone(),
                amount: r.launch_amount.to_string(),
                time: r.launch_time,
            }).collect::<Vec<_>>();
            let data = MintRecordsRsp {
              page_count, mint_records
            };
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(data)
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_launch_records failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get mint records failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}

pub async fn get_account_invitees(data: web::Data<AppState>, req: HttpRequest)
                              -> actix_web::Result<HttpResponse> {
    let query_str = req.query_string();
    let qs = QString::from(query_str);
    let pg_no = qs.get("pg_no").unwrap_or("1").parse::<i32>().unwrap();
    let Some(address) = qs.get("address") else {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Not input address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    };

    let Some(address) = get_solana_address_from_parameter(address,&data.db).await else {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Need bind solana address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    };

    match db::get_account_invitees(&data.db,&address,pg_no).await {
        Ok((page_count,records)) => {
            let invitees = records.iter().map(|r| {
                let rebate = BigDecimal::from_str(&r.mint_amount.to_string()).unwrap().div(BigDecimal::from(10u32));
                AccountInvitee {
                    invitee: r.address.clone(),
                    mint_amount: r.mint_amount.to_string(),
                    rebate: rebate.to_string(),
                }
            }).collect::<Vec<_>>();
            let data = AccountInviteesRsp {
                page_count,
                invitees,
            };
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(data)
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_account_invitees failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get account invitees failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}

pub async fn get_account_invitees_count(data: web::Data<AppState>, req: HttpRequest)
                                  -> actix_web::Result<HttpResponse> {
    let query_str = req.query_string();
    let qs = QString::from(query_str);
    let Some(address) = qs.get("address") else {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Not input address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    };

    let Some(address) = get_solana_address_from_parameter(address,&data.db).await else {
        let resp = BackendResponse {
            code: BackendError::InvalidParameters,
            error: Some("Need bind solana address".to_owned()),
            data: None::<()>
        };
        return Ok(HttpResponse::Ok().json(resp));
    };

    match db::get_account_invitees_count(&data.db,&address).await {
        Ok(count) => {
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(count)
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_account_invitees_count failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get account invitees failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}

pub async fn get_queried_addresses_number(data: web::Data<AppState>, _req: HttpRequest)
                                          -> actix_web::Result<HttpResponse> {
    match db::db_get_queried_addresses_number(&data.db).await {
        Ok(query_number) => {
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(query_number)
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_queried_addresses_number failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get_queried_addresses_number failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}

pub async fn get_total_claimed_number(data: web::Data<AppState>, _req: HttpRequest)
                                      -> actix_web::Result<HttpResponse> {
    match db::db_get_total_claimed_number(&data.db).await {
        Ok(query_number) => {
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(query_number)
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_total_claimed_number failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get_total_claimed_number failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}

pub async fn get_total_claimed_amount(data: web::Data<AppState>, _req: HttpRequest)
                                      -> actix_web::Result<HttpResponse> {
    match db::db_get_total_claimed_amount(&data.db).await {
        Ok(amount) => {
            let amount = BigDecimal::from_str(&amount.0.to_string()).unwrap();
            let resp = BackendResponse {
                code: BackendError::Ok,
                error: None,
                data: Some(amount.to_string())
            };
            Ok(HttpResponse::Ok().json(resp))
        },
        Err(e) => {
            log::warn!("get_total_claimed_amount failed,{e}");
            let resp = BackendResponse {
                code: BackendError::InternalErr,
                error: Some("get_total_claimed_amount failed".to_owned()),
                data: None::<()>
            };
            Ok(HttpResponse::Ok().json(resp))
        }
    }
}