-- Your SQL goes here
CREATE TABLE query_accounts (
     address text NOT NULL,
     claimable_amount numeric NOT NULL,
     query_time bigint NOT NULL,
     claim_sol_address text,
     PRIMARY KEY (address)
);

CREATE TABLE accounts (
     address text NOT NULL, -- solana address
     invite_code text NOT NULL,
     inviter text,
     create_time bigint NOT NULL,
     point bigint NOT NULL,
     PRIMARY KEY (address)
);

CREATE TABLE launch_records (
     address text NOT NULL, -- solana address
     launch_amount numeric NOT NULL,
     launch_block bigint NOT NULL,
     launch_tx_hash text NOT NULL,
     launch_time bigint NOT NULL,
     log_index smallint NOT NULL,
     PRIMARY KEY (launch_tx_hash,log_index)
);

CREATE TABLE last_sync_block (
    block_number bigint NOT NULL,
    PRIMARY KEY (block_number)
);

