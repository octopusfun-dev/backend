use std::cmp;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::ops::Div;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use base58::FromBase58;
use bigdecimal::BigDecimal;
use borsh::{BorshDeserialize, BorshSerialize};
use itertools::Itertools;
use rayon::prelude::IntoParallelRefIterator;
use rbatis::rbdc::Decimal;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcBlockConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{system_instruction, system_program};
use solana_sdk::system_instruction::SystemInstruction;
use tokio::task::JoinHandle;
use crate::config::Config;
use crate::db;
use solana_transaction_status::{EncodedTransactionWithStatusMeta, TransactionDetails, UiConfirmedBlock, UiTransactionEncoding};
use solana_transaction_status::UiInstruction::Compiled;
use crate::db::tables::LaunchRecord;
use rayon::iter::ParallelIterator;
use tokio::sync::Mutex as TokioMutex;

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq)]
pub struct Mint {
    pub amount: u64,
    pub bump: u8,
}
#[derive(Clone)]
pub struct ChainWatcher {
    pub config: Config,
    pub client: Arc<RpcClient>,
    pub db: rbatis::RBatis,
    pub blocks_queue: Arc<TokioMutex<VecDeque<UiConfirmedBlock>>>,
}
fn parse_transfer_logs(transactions:Vec<EncodedTransactionWithStatusMeta>,
                       slot: i64,
                       block_time: i64,
) ->Vec<LaunchRecord> {
    let records = Arc::new(Mutex::new(vec![]));
    transactions.par_iter().for_each(|tx| {
        let Some(decoded_tx) = tx.transaction.decode() else {
            return;
        };
        let Some(ref meta) = tx.meta else {
            return;
        };
        if meta.err.is_some() {
            return;
        }

        let logs = meta.log_messages.clone().unwrap_or(vec![]);
        if logs.is_empty() {
            return;
        }
        let positions_log_start: Vec<_> = logs
            .iter()
            .positions(|l|
                *l == "Program Bdro1T9cT2ZroyJdHFCnrchx45L4Vf87NUhQY1pVD1Qm invoke [1]")
            .collect();
        if !positions_log_start.is_empty() {
            let positions_log_end: Vec<_> = logs
                .iter()
                .positions(|l|
                    *l == "Program Bdro1T9cT2ZroyJdHFCnrchx45L4Vf87NUhQY1pVD1Qm success")
                .collect();
            if positions_log_start.len() != positions_log_end.len() {
                panic!("invalid log data");
            }

            for (si, s) in positions_log_start.iter().enumerate() {
                let e = positions_log_end[si];
                let mut log_index = 0;
                for i in s + 1..e {
                    let mint_log_tip = "Program log: Mint user = ";
                    if logs[i].contains(mint_log_tip) {
                        let mint_metas = logs[i].split(',').collect::<Vec<_>>();
                        let account_from = mint_metas[0].split('=').collect::<Vec<_>>()[1].trim();
                        let amount_str = mint_metas[1].split('=').collect::<Vec<_>>()[1].trim();
                        let sol_amount = BigDecimal::from_str(amount_str)
                            .unwrap_or(BigDecimal::from(0))
                            .div(BigDecimal::from(100000000));
                        log::info!("Get mint event from {:?} buy {:?} lamport at slot {} tx {}",
                                                 account_from,sol_amount, slot, decoded_tx.signatures[0].to_string());
                        records.lock().unwrap().push(LaunchRecord {
                            address: account_from.to_string(),
                            launch_amount: Decimal::from_str(&sol_amount.to_string()).unwrap(),
                            launch_block: slot,
                            launch_tx_hash: decoded_tx.signatures[0].to_string(),
                            log_index,
                            launch_time: block_time,
                        });
                        log_index += 1;
                    }
                }
            }
        }
    });
    let records = records.lock().unwrap().to_vec();
    records
}
impl ChainWatcher {
    pub fn new(config:Config,db: rbatis::RBatis) -> Self {
        let client = Arc::new(RpcClient::new(config.remote_web3_url.clone()));
        let blocks_queue = Arc::new(TokioMutex::new(VecDeque::new()));
        Self {
            config,
            client,
            db,
            blocks_queue,
        }
    }

    async fn run_sync_transfers(&mut self) ->anyhow::Result<()> {
        let last_synced_block = db::get_last_sync_block(&self.db,self.config.sync_start_block).await?;
        let chain_block_number = self.client.get_slot().await?;
        println!("run_sync_transfers last_synced_block from db is {last_synced_block},last block on chain is {chain_block_number}");
        let sync_step = 100u64;
        let mut start_block = last_synced_block + 1;
        let mut end_block;
        loop {
            end_block = cmp::min(chain_block_number,start_block + sync_step);
            println!("sync loop {start_block} - {end_block}");
            if start_block > end_block {
                break;
            }
            let mut records = vec![];
            let slots = self.client.get_blocks(start_block,Some(end_block)).await?;
            for slot in slots {
                let config = RpcBlockConfig {
                    encoding: Some(UiTransactionEncoding::Binary),
                    transaction_details: Some(TransactionDetails::Full),
                    rewards: Some(false),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                };
                let block = self.client.get_block_with_config(slot, config).await?;
                let Some(transactions) = block.transactions else {
                    continue;
                };
                for tx in transactions {
                    let Some(decoded_tx) = tx.transaction.decode() else {
                        continue;
                    };
                    let Some(meta) = tx.meta else {
                        continue;
                    };
                    if meta.err.is_some() {
                        continue;
                    }
                    for ins in decoded_tx.message.instructions() {
                        let account_keys = decoded_tx.message.static_account_keys();
                        let program_id = account_keys[ins.program_id_index as usize];
                        if program_id != Pubkey::from_str(&self.config.launch_program_id).unwrap() {
                            continue;
                        }
                        if let Err(_) = borsh::from_slice::<Mint>(&ins.data) {
                            continue;
                        }
                        for inner_ins in meta.clone().inner_instructions.unwrap() {
                            for iins in inner_ins.instructions {
                                match iins {
                                    Compiled(compiled) => {
                                        let program_id = decoded_tx.message.static_account_keys()[compiled.program_id_index as usize];
                                        let account_from = decoded_tx.message.static_account_keys()[compiled.accounts[0] as usize];
                                        let account_to = decoded_tx.message.static_account_keys()[compiled.accounts[1] as usize];
                                        if program_id != system_program::ID || account_to !=
                                            Pubkey::from_str(&self.config.receiver_address).unwrap() {
                                            continue;
                                        }

                                        let decoded_bytes = compiled.data.from_base58().unwrap();
                                        let Ok(system_ins) =
                                            bincode::deserialize::<SystemInstruction>(&decoded_bytes) else {
                                            continue;
                                        };
                                        let SystemInstruction::Transfer { lamports } = system_ins else {
                                            continue;
                                        };

                                        let sol_amount = BigDecimal::from(lamports).div(BigDecimal::from(100000000));
                                        log::info!("there is sol transfer {:?} from {:?} to {:?} at slot {} tx {}",
                                                 sol_amount, account_from, account_to, slot, decoded_tx.signatures[0].to_string());
                                        records.push(LaunchRecord {
                                            address: account_from.to_string(),
                                            launch_amount: Decimal::from_str(&sol_amount.to_string()).unwrap(),
                                            launch_block: slot as i64,
                                            launch_tx_hash: decoded_tx.signatures[0].to_string(),
                                            log_index: 0,
                                            launch_time: block.block_time.unwrap_or_default(),
                                        })
                                    }
                                    _ => {
                                        continue;
                                    }
                                }
                            }
                        }

                    }
                }
            }
            db::save_launch_records(&mut self.db,&records).await?;
            start_block = end_block + 1;
            db::upsert_last_sync_block(
                &mut self.db,
                end_block as i64,
            ).await?;
        }
        Ok(())
    }

    async fn get_blocks(&mut self) ->anyhow::Result<()> {
        let last_synced_block = db::get_last_sync_block(&self.db,self.config.sync_start_block).await?;
        let chain_block_number = self.client.get_slot().await?;
        println!("run_sync_transfers last_synced_block from db is {last_synced_block},last block on chain is {chain_block_number}");
        let sync_step = 1000u64;
        let mut start_block = last_synced_block + 1;
        let mut end_block;
        loop {
            end_block = cmp::min(chain_block_number,start_block + sync_step);
            println!("sync loop {start_block} - {end_block}");
            if start_block > end_block {
                break;
            }
            let slots = self.client.get_blocks(start_block,Some(end_block)).await?;
            for slot in slots {
                let config = RpcBlockConfig {
                    encoding: Some(UiTransactionEncoding::Binary),
                    transaction_details: Some(TransactionDetails::Full),
                    rewards: Some(false),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                };
                let block = self.client.get_block_with_config(slot, config).await?;
                self.blocks_queue.lock().await.push_back(block);
            }
            start_block = end_block + 1;
        }
        Ok(())
    }

    async fn run_sync_transfers_logs(mut self) ->anyhow::Result<()> {
        // let mut records = vec![];
        loop {
            let mut queue = self.blocks_queue.lock().await;
            if let Some(block) = queue.pop_front() {
                log::info!("process block {:?} transfer logs",block.block_height);
                let Some(transactions) = block.transactions else {
                    continue;
                };
                let records = parse_transfer_logs(transactions,
                                                       block.block_height.unwrap_or_default() as i64,
                                                       block.block_time.unwrap_or_default());
                if !records.is_empty() {
                    log::info!("get mint records in block {:?}",block.block_height);
                    db::save_launch_records(&mut self.db, &records).await?;
                }
            } else {
                log::info!("no block need to process");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    pub async fn run_get_blocks_server(mut self) {

        let mut tx_poll = tokio::time::interval(Duration::from_secs(1));
        loop {
            tx_poll.tick().await;
            if self.config.claim_start {
                if let Err(e) = self.get_blocks().await {
                    log::error!("get_blocks error occurred {:?}", e);
                }
            }

        }
    }
}
pub async fn run_watcher(config: Config, db: rbatis::RBatis) -> JoinHandle<()> {
    log::info!("Starting watcher!");
    let mut watcher = ChainWatcher::new(config, db);
    tokio::spawn(watcher.clone().run_sync_transfers_logs());
    tokio::spawn(watcher.run_get_blocks_server())
}

#[cfg(test)]
mod test {
    use base58::FromBase58;
    use itertools::Itertools;
    use solana_client::rpc_config::RpcBlockConfig;
    use solana_sdk::commitment_config::CommitmentConfig;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::system_instruction;
    use solana_transaction_status::{EncodedTransaction, UiInstruction, UiMessage, UiParsedInstruction};
    use solana_transaction_status::UiInstruction::Compiled;
    use super::*;

    #[tokio::test]
    async fn test_solana_tx() {
        let client = RpcClient::new("https://api.devnet.solana.com".to_string());
        let chain_block_number = client.get_slot().await.unwrap();
        println!("current block number on chain is {:?}", chain_block_number);
        //let slots = client.get_blocks(254693348,Some(254693348+1)).await.unwrap();
        let slot = 349247008;
        //let slot = 347992071;
        //for slot in slots {
        let config = RpcBlockConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            transaction_details: Some(TransactionDetails::Full),
            rewards: Some(false),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };
        let block = client.get_block_with_config(slot, config).await.unwrap();
        for tx in block.transactions.unwrap() {
            let Some(tx_meta) = tx.meta.clone() else {
                continue;
            };
            let logs = tx_meta.log_messages.unwrap_or(vec![]);
            if logs.is_empty() {
                continue;
            }
            // loop {
            let positions_log_start: Vec<_> = logs.iter().positions(|l| *l == "Program Bdro1T9cT2ZroyJdHFCnrchx45L4Vf87NUhQY1pVD1Qm invoke [1]").collect();
            if !positions_log_start.is_empty() {
                println!("found octo log {:?}", positions_log_start);
                let positions_log_end: Vec<_> = logs.iter().positions(|l| *l == "Program Bdro1T9cT2ZroyJdHFCnrchx45L4Vf87NUhQY1pVD1Qm success").collect();
                if positions_log_start.len() != positions_log_end.len() {
                    panic!("invalid log data");
                }

                let _ = positions_log_start
                    .iter()
                    .zip(positions_log_end)
                    .map(|(s, e)| {
                        for i in s + 1..e {
                            let mint_log_tip = "Program log: Mint user = ";
                            if logs[i].contains(mint_log_tip) {
                                println!("found mint log");
                                let mint_metas = logs[i].split(',').collect::<Vec<_>>();
                                let account_from = mint_metas[0].split('=').collect::<Vec<_>>()[1].trim();
                                let amount_str = mint_metas[1].split('=').collect::<Vec<_>>()[1].trim();
                                let sol_amount = BigDecimal::from_str(amount_str).unwrap_or(BigDecimal::from(0));
                                println!("Get mint event from {:?} buy {:?} lamport at slot {}",
                                         account_from, sol_amount, slot);
                            }
                        }
                    }).collect::<Vec<_>>();
            }
            //}
            //logs.iter().find()
        }
    }

    #[tokio::test]
    async fn test_get_blocks() {
        let client = Arc::new(RpcClient::new("https://api.devnet.solana.com".to_string()));
        let start_block = 349247008;
        let end_block = start_block + 100;
        let slots = client.get_blocks(start_block, Some(end_block)).await.unwrap();
        //let mut blocks = vec![];
        let mut tasks = vec![];
        for slot in slots {
            let config = RpcBlockConfig {
                encoding: Some(UiTransactionEncoding::Binary),
                transaction_details: Some(TransactionDetails::Full),
                rewards: Some(false),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
            };
            let client = client.clone();
            tasks.push(tokio::spawn(async move {
                client.get_block_with_config(slot, config).await
            }));
            // println!("get block from rpc: {slot}");
            //blocks.push(block);
        }

        for task in tasks {
            if let Ok(block) = task.await.unwrap() {
                println!("get block from rpc {:?}",block.block_height)
            }
        }
    }
}