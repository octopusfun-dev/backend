use std::str::FromStr;
use solana_sdk::pubkey::Pubkey;
use crate::db;
pub async fn get_solana_address_from_parameter(addr_str:&str,db:&rbatis::RBatis) -> Option<String> {
    let solana_address = if let Ok(solana_key) = Pubkey::from_str(addr_str) {
        Some(solana_key.to_string())
    } else {
        let mut evm_address = addr_str.to_lowercase();
        if let Some(ret) = evm_address.strip_prefix("0x") {
            evm_address = ret.to_string();
        };
        if let Ok(address) = db::get_user_bind_sol_address(db,&evm_address).await {
            address
        } else {
            None
        }
    };
    solana_address
}