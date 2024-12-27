use std::default;
use rbatis::rbdc::decimal::Decimal;
use std::str::FromStr;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LastSyncBlock {
    pub block_number: i64,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct QueryAccount {
    pub address: String,
    pub claimable_amount: Decimal,
    pub claim_sol_address: Option<String>,
    pub query_time: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Account {
    pub address: String,
    pub invite_code: String,
    pub inviter: Option<String>,
    pub create_time: i64,
    pub point: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AccountInviteeInfo {
    pub address: String,
    pub mint_amount: Decimal,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LaunchRecord {
    pub address: String,
    pub launch_amount: Decimal,
    pub launch_block: i64,
    pub launch_tx_hash: String,
    pub log_index: i32,
    pub launch_time: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct UserPoint {
    pub address: String,
    pub point: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AccountEligible {
    pub address: String,
    pub claimable_amount: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClaimedAccount {
    pub address: String,
    pub claimed_time: i64,
    pub claimed_amount: Decimal,
}

rbatis::crud!(QueryAccount {}, "query_accounts");
rbatis::crud!(ClaimedAccount {}, "claimed_accounts");
rbatis::crud!(LastSyncBlock {}, "last_sync_block");
rbatis::crud!(Account {}, "accounts");
rbatis::crud!(LaunchRecord {}, "launch_records");

impl Default for QueryAccount {
    fn default() -> Self {
        QueryAccount {
            //address: H160::zero().to_string(),
            address:"".to_string(),
            claimable_amount: Decimal::from_str("0").unwrap(),
            claim_sol_address: None,
            query_time: 0,
        }
    }
}
// impl From<ClaimEvent> for ClaimedAccount {
//     fn from(event: ClaimEvent) -> Self {
//         Self {
//             address: event.address.to_string(),
//             claimed_time: event.claimed_time.as_u64() as i64,
//             claimed_amount: Decimal::from_str(&event.amount.to_string()).unwrap_or(Decimal::from_str("0").unwrap()),
//         }
//     }
// }
