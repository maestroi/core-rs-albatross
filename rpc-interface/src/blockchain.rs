use async_trait::async_trait;

use futures::stream::BoxStream;
use nimiq_hash::Blake2bHash;
use nimiq_keys::Address;
use nimiq_primitives::coin::Coin;
use std::collections::HashMap;

use crate::types::{Account, Block, Inherent, SlashedSlots, Slot, Staker, Transaction, Validator};

#[cfg_attr(
    feature = "proxy",
    nimiq_jsonrpc_derive::proxy(name = "BlockchainProxy", rename_all = "camelCase")
)]
#[async_trait]
pub trait BlockchainInterface {
    type Error;

    async fn get_block_number(&mut self) -> Result<u32, Self::Error>;

    async fn get_epoch_number(&mut self) -> Result<u32, Self::Error>;

    async fn get_batch_number(&mut self) -> Result<u32, Self::Error>;

    async fn get_block_by_hash(
        &mut self,
        hash: Blake2bHash,
        include_transactions: bool,
    ) -> Result<Block, Self::Error>;

    async fn get_block_by_number(
        &mut self,
        block_number: u32,
        include_transactions: bool,
    ) -> Result<Block, Self::Error>;

    async fn get_latest_block(&mut self, include_transactions: bool) -> Result<Block, Self::Error>;

    async fn get_slot_at(
        &mut self,
        block_number: u32,
        view_number: Option<u32>,
    ) -> Result<Slot, Self::Error>;

    // TODO: Previously called `slot_state`. Where is this used?
    async fn get_slashed_slots(&mut self) -> Result<SlashedSlots, Self::Error>;

    async fn get_raw_transaction_info(&mut self, raw_tx: String) -> Result<(), Self::Error>;

    async fn get_transaction_by_hash(
        &mut self,
        hash: Blake2bHash,
    ) -> Result<Transaction, Self::Error>;

    async fn get_transactions_by_block_number(
        &mut self,
        block_number: u32,
    ) -> Result<Vec<Transaction>, Self::Error>;

    async fn get_batch_inherents(
        &mut self,
        batch_number: u32,
    ) -> Result<Vec<Inherent>, Self::Error>;

    async fn get_transaction_receipt(&mut self, hash: Blake2bHash) -> Result<(), Self::Error>;

    async fn get_transaction_hashes_by_address(
        &mut self,
        address: Address,
        max: Option<u16>,
    ) -> Result<Vec<Blake2bHash>, Self::Error>;

    async fn get_transactions_by_address(
        &mut self,
        address: Address,
        max: Option<u16>,
    ) -> Result<Vec<Transaction>, Self::Error>;

    async fn list_stakes(&mut self) -> Result<HashMap<Address, Coin>, Self::Error>;

    async fn get_validator(
        &mut self,
        address: Address,
        include_stakers: Option<bool>,
    ) -> Result<Validator, Self::Error>;

    async fn get_staker(&mut self, address: Address) -> Result<Staker, Self::Error>;

    #[stream]
    async fn head_subscribe(&mut self) -> Result<BoxStream<'static, Blake2bHash>, Self::Error>;

    async fn get_account(&mut self, address: Address) -> Result<Account, Self::Error>;
}
