use std::collections::HashMap;
use num::ToPrimitive;
use rbatis::RBatis;
use rbatis::rbdc::decimal::Decimal;
use solana_sdk::pubkey::new_rand;
use crate::db::tables::{Account, AccountEligible, AccountInviteeInfo, ClaimedAccount, LastSyncBlock, LaunchRecord, QueryAccount, UserPoint};

pub(crate) mod tables;

pub(crate) async fn upsert_last_sync_block(rb: &mut RBatis, new_block : i64) -> anyhow::Result<()> {
    let block = LastSyncBlock::select_all(rb).await?;
    if block.is_empty() {
        rb.exec("insert into last_sync_block values (?)",
                vec![rbs::to_value!(new_block)])
            .await?;
    } else {
        rb.exec("update last_sync_block set block_number = ?",
                vec![rbs::to_value!(new_block)])
            .await?;
    }
    Ok(())
}

pub async fn get_last_sync_block(rb:&RBatis,start_block: u64) -> anyhow::Result<u64> {
    let block: Vec<LastSyncBlock> = rb
        .query_decode("select block_number from last_sync_block",vec![])
        .await?;
    let number = if block.is_empty() {
        start_block
    } else {
        block[0].block_number.to_u64().unwrap()
    };
    Ok(number)
}

pub(crate) async fn save_account(rb: &mut RBatis, account: &Account) -> anyhow::Result<()> {
    println!("New account is {:?}",account);
    rb.exec("insert into accounts (address,invite_code,inviter,create_time) values (?,?,?,CURRENT_TIMESTAMP) on conflict (address) do nothing",
            vec![rbs::to_value!(account.address.clone()),
                 rbs::to_value!(account.invite_code.clone()),
                 rbs::to_value!(account.inviter.clone()),
            ]).await?;

    Ok(())
}

pub(crate) async fn update_inviter_point(rb: &mut RBatis, inviter: &str,add_point: i64) -> anyhow::Result<()> {
    rb.exec("update accounts set point = point + 1000 where address = ?",
            vec![rbs::to_value!(inviter), rbs::to_value!(add_point)]).await?;
    Ok(())
}

pub async fn get_query_account_by_address(rb:&RBatis,address: String) -> anyhow::Result<Option<QueryAccount>> {
    let account: Option<QueryAccount> = rb
        .query_decode("select * from query_accounts where address = ? limit 1",vec![rbs::to_value!(address)])
        .await?;
    Ok(account)
}

// pub async fn get_user_bind_evm_address(rb:&RBatis,address: &str) -> anyhow::Result<Option<String>> {
//     let account: Option<Account> = rb
//         .query_decode("select * from query_accounts where claim_sol_address = ?",vec![rbs::to_value!(address)])
//         .await?;
//     Ok(account.map(|a|a.address))
// }

pub async fn get_user_bind_sol_address(rb:&RBatis,address: &str) -> anyhow::Result<Option<String>> {
    let account: Option<QueryAccount> = rb
        .query_decode("select * from query_accounts where address = ? limit 1",vec![rbs::to_value!(address)])
        .await?;
    let sol_address = if let Some(account) = account {
        account.claim_sol_address
    } else {
        None
    };
    Ok(sol_address)
}

pub async fn get_account_by_address(rb:&RBatis,address: &str) -> anyhow::Result<Option<Account>> {
    let account: Option<Account> = rb
        .query_decode("select * from accounts where address = ? limit 1",vec![rbs::to_value!(address)])
        .await?;
    Ok(account)
}

pub async fn get_account_by_inviter_code(rb:&RBatis,code: &str) -> anyhow::Result<Option<Account>> {
    let account: Option<Account> = rb
        .query_decode("select * from accounts where invite_code = ? limit 1",vec![rbs::to_value!(code)])
        .await?;
    Ok(account)
}
pub async fn get_account_invitees(rb:&RBatis,address: &str,page_no: i32) -> anyhow::Result<(usize,Vec<AccountInviteeInfo>)> {
    let PAGE_SIZE = 10;
    let offset = (page_no - 1) * PAGE_SIZE;
    let invitees: Vec<AccountInviteeInfo> = rb
        .query_decode("select l.address,sum(l.launch_amount) as mint_amount from accounts a \
        join launch_records l \
        on a.address = l.address \
        where a.inviter = ? group by l.address order by mint_amount desc offset ? limit ?",
                      vec![rbs::to_value!(address),rbs::to_value!(offset),rbs::to_value!(PAGE_SIZE)])
        .await?;
    println!("invitees is {:?}",invitees);
    let count: HashMap<String,usize> = rb.query_decode("select count(1) from accounts a \
        join launch_records l \
        on a.address = l.address \
        where a.inviter = ?",
        vec![rbs::to_value!(address)]).await?;
    let count = count.get("count").unwrap();
    let pg_count = count / PAGE_SIZE as usize;
    Ok((pg_count,invitees))
}

pub async fn get_account_invitees_count(rb:&RBatis,address: &str) -> anyhow::Result<usize> {
    let ret: HashMap<String,usize> = rb
        .query_decode("select count(1) from accounts where inviter = ?",
                      vec![rbs::to_value!(address)])
        .await?;
    let count = ret.get("count").unwrap();
    Ok(*count)
}

pub async fn get_account_invitees_total_mint(rb:&RBatis,address: &str) -> anyhow::Result<Decimal> {
    let total_mint: Decimal = rb
        .query_decode("select coalesce(sum(lr.launch_amount),0) as total_amount from launch_records lr \
         join accounts a on lr.address = a.address
         where a.inviter = ?",vec![rbs::to_value!(address)])
        .await?;
    Ok(total_mint)
}

pub async fn get_total_mint(rb:&RBatis) -> anyhow::Result<Decimal> {
    let total_mint: Decimal = rb
        .query_decode("select coalesce(sum(launch_amount),0) as total_amount from launch_records",
                      vec![])
        .await?;
    Ok(total_mint)
}

pub async fn get_total_rebate(rb:&RBatis) -> anyhow::Result<Decimal> {
    let total_mint: Decimal = rb
        .query_decode("select coalesce(sum(lr.launch_amount),0) as total_amount from launch_records lr \
         join accounts a on lr.address = a.address
         where a.inviter is not null",
                      vec![])
        .await?;
    Ok(total_mint)
}
pub(crate) async fn save_query_account(rb: &mut RBatis, query: QueryAccount) -> anyhow::Result<()> {
    println!("query is {:?}",query);
    rb.exec("insert into query_accounts (address,claimable_amount,query_time) \
        values (?,?,?) on conflict(address) do nothing",
            vec![rbs::to_value!(query.address),
                 rbs::to_value![query.claimable_amount.clone()],
                 rbs::to_value!(query.query_time.clone()),
            ]).await?;

    Ok(())
}

pub(crate) async fn update_query_account_sol_address(rb: &mut RBatis, address:&str,sol_address:&str) -> anyhow::Result<()> {
    rb.exec("update query_accounts set claim_sol_address = ? where address = ? ",
            vec![rbs::to_value!(sol_address),
                 rbs::to_value!(address),
            ]).await?;
    Ok(())
}
pub async fn get_queried_account(rb: &RBatis,address: &str) ->anyhow::Result<Option<QueryAccount>> {
    let ret: Option<QueryAccount> = rb
        .query_decode("select * from query_accounts where address = ? limit 1 ",vec![rbs::to_value!(address)])
        .await?;
    println!("get_queried_account ret is {:?}",ret);
    Ok(ret)
}
pub async fn get_all_queried_accounts(rb: &RBatis) ->anyhow::Result<Vec<AccountEligible>> {
    let ret: Vec<QueryAccount> = rb
        .query_decode("select * from query_accounts order by address asc",vec![])
        .await?;
    let accounts_eligible = ret.iter().map(|a| AccountEligible {
        address: a.address.clone(),
        claimable_amount: a.claimable_amount.0.to_string(),
    }).collect::<Vec<_>>();
    Ok(accounts_eligible)
}

pub(crate) async fn save_claimed_accounts(rb: &mut RBatis, accounts: Vec<ClaimedAccount>) -> anyhow::Result<()> {
    for account in accounts {
        rb.exec("insert into claimed_accounts (address,claimed_time,claimed_amount) \
        values (?,?,?) on conflict(address) do nothing",
                vec![rbs::to_value!(account.address),
                     rbs::to_value!(account.claimed_time.clone()),
                     rbs::to_value!(account.claimed_amount.clone()),
                ]).await?;
    }
    Ok(())
}

pub(crate) async fn save_launch_records(rb: &mut RBatis, records: &Vec<LaunchRecord>) -> anyhow::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let mut sql_str = "insert into launch_records \
    (address,launch_amount,launch_block,launch_tx_hash,log_index,launch_time) values ".to_string();
    for record in records {
        let s = format!("('{}',{},{},'{}',{},{}),",record.address,record.launch_amount,
                        record.launch_block,record.launch_tx_hash,record.log_index,record.launch_time);
        sql_str += &s;
    }
    sql_str.truncate(sql_str.len() - 1);
    sql_str += " on conflict (launch_tx_hash,log_index) do nothing";
    println!("save_launch_records sql {sql_str}");
    rb.exec(&sql_str,vec![]).await?;
    Ok(())
}

pub async fn get_launch_records(rb: &RBatis,page_no:i32) -> anyhow::Result<(usize,Vec<LaunchRecord>)> {
    let PAGE_SIZE = 10;
    let offset = (page_no - 1) * PAGE_SIZE;
    let ret: Vec<LaunchRecord> = rb
        .query_decode("select * from launch_records order by launch_time desc offset ? limit ? ",
                      vec![rbs::to_value!(offset),rbs::to_value!(PAGE_SIZE)])
        .await?;
    let count: HashMap<String,usize> = rb
        .query_decode("select count(1) from launch_records",vec![]).await?;
    let count = count.get("count").unwrap();
    let pg_count = count / PAGE_SIZE as usize;
    Ok((pg_count,ret))
}

pub async fn get_pre_launch_record(rb: &RBatis,address: &str,launch_time:i64) -> anyhow::Result<Vec<LaunchRecord>> {
    let ret: Vec<LaunchRecord> = rb
        .query_decode("select *  from launch_records where address = ? and launch_time < ? ",
                      vec![rbs::to_value!(address),rbs::to_value!(launch_time)])
        .await?;
    Ok(ret)
}

pub async fn get_accounts(rb: &RBatis,addresses: Vec<String>) ->anyhow::Result<Vec<Account>> {
    let mut sql_str = "select * from accounts where address in (".to_string();
    for address in addresses {
        sql_str += &format!("'{}',",address);
    }
    sql_str.truncate(sql_str.len() - 1 );
    sql_str += ")";
    let ret: Vec<Account> = rb
        .query_decode(&sql_str,vec![])
        .await?;
    Ok(ret)
}

pub(crate) async fn update_user_points(rb: &mut RBatis, records: Vec<UserPoint>) -> anyhow::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let mut sql_str = "update accounts set point = t.point from (values ".to_string();
    for record in records {
        let v = format!("('{}',{}),",record.address,record.point);
        sql_str += &v;
    }
    sql_str.truncate(sql_str.len() - 1);
    sql_str += ") as t (address,point) where accounts.address = t.address";
    rb.exec(&sql_str,vec![]).await?;
    Ok(())
}

pub(crate) async fn db_bind_sol_address(rb: &mut RBatis, query_account: Option<QueryAccount>,
                                        new_account: Option<Account>,inviter_address: Option<String>) -> anyhow::Result<()> {
    let tx = rb.acquire_begin().await?;
    //1.update the sol address of query account
    if let Some(query_account) = query_account {
        tx.exec("update query_accounts set claim_sol_address = ? where address = ? ",
                vec![rbs::to_value!(&query_account.claim_sol_address),
                     rbs::to_value!(query_account.address),
                ]).await?;
    }
    //2.save new account
    if let Some(new_account) = new_account {
        tx.exec("insert into accounts (address,invite_code,inviter,create_time,point) values (?,?,?,?,?) on conflict (address) do nothing",
                vec![rbs::to_value!(new_account.address.clone()),
                     rbs::to_value!(new_account.invite_code.clone()),
                     rbs::to_value!(new_account.inviter.clone()),
                     rbs::to_value!(new_account.create_time),
                     rbs::to_value!(new_account.point),
                ]).await?;
    }
    //3.update inviter's point
    if let Some(inviter_address) = inviter_address {
        tx.exec("update accounts set point = point + ? where address = ?",
                vec![ rbs::to_value!(1000),rbs::to_value!(inviter_address)]).await?;
    }
    tx.commit().await?;
    Ok(())
}
pub async fn db_get_queried_addresses_number(rb:&RBatis) -> anyhow::Result<u64> {
    let queried_number: u64 = rb
        .query_decode("select count(1) from query_accounts",vec![])
        .await?;
    Ok(queried_number)
}
pub async fn db_get_total_claimed_number(rb:&RBatis) -> anyhow::Result<u64> {
    let claimed_number: u64 = rb
        .query_decode("select count(1) from claimed_accounts",vec![])
        .await?;
    Ok(claimed_number)
}
pub async fn db_get_total_claimed_amount(rb:&RBatis) -> anyhow::Result<Decimal> {
    let claimed_number: Decimal = rb
        .query_decode("select sum(claimed_amount) from claimed_accounts",vec![])
        .await?;
    Ok(claimed_number)
}