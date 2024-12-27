use std::env;
#[derive(Debug,Clone)]
pub struct Config {
    pub port: u16,
    pub workers: u16,
    pub token_address: String,
    pub token_decimal: u32,
    pub database_url: String,
    pub db_pool_size: u16,
    pub remote_web3_url: String,
    pub sync_start_block: u64,
    pub claim_start: bool,
    pub receiver_address: String,
    pub launch_program_id: String,
    pub launch_max_amount: u64,
}

impl Config {
    pub fn from_env() ->Self {
        let port = env::var("SERVER_PORT").unwrap_or_default()
            .parse::<u16>().unwrap_or(8088u16);
        let workers = env::var("WORKERS_NUMBER").unwrap_or_default()
            .parse::<u16>().unwrap_or(2u16);
        let token_address = env::var("TOKEN_ADDRESS").unwrap_or_default();

        let database_url = env::var("DATABASE_URL").unwrap_or_default();
        let remote_web3_url = env::var("REMOTE_WEB3_URL").unwrap_or_default();
        let db_pool_size = env::var("DB_POOL_SIZE").unwrap_or_default()
            .parse::<u16>().unwrap_or(1u16);
        let sync_start_block = env::var("SYNC_START_BLOCK").unwrap_or_default()
            .parse::<u64>().unwrap_or(0u64);
        let token_decimal = env::var("TOKEN_DECIMAL").unwrap_or_default()
            .parse::<u32>().unwrap_or(0u32);
        let claim_start = env::var("CLAIM_START").unwrap_or_default()
            .parse::<bool>().unwrap_or(false);
        let receiver_address = env::var("RECEIVER_ADDRESS").unwrap_or_default();
        let launch_program_id = env::var("LAUNCH_PROGRAM_ID").unwrap_or_default();
        let launch_max_amount = env::var("LAUNCH_MAX_AMOUNT").unwrap_or_default()
            .parse::<u64>().unwrap_or(0u64);
        Self {
            port,
            workers,
            token_address,
            token_decimal,
            database_url,
            db_pool_size,
            remote_web3_url,
            sync_start_block,
            claim_start,
            receiver_address,
            launch_program_id,
            launch_max_amount
        }
    }
}