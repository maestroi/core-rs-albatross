use std::collections::{BTreeMap, BTreeSet};

use beserial::{Deserialize, Serialize};
use nimiq_collections::BitSet;
use nimiq_database::WriteTransaction;
use nimiq_keys::Address;
use nimiq_primitives::coin::Coin;
use nimiq_primitives::policy;
use nimiq_primitives::slots::SlashedSlot;
use nimiq_transaction::account::staking_contract::{
    IncomingStakingTransactionData, OutgoingStakingTransactionProof,
};
use nimiq_transaction::Transaction;

use crate::interaction_traits::{AccountInherentInteraction, AccountTransactionInteraction};
use crate::staking_contract::receipts::DropValidatorReceipt;
use crate::staking_contract::SlashReceipt;
use crate::{Account, AccountError, AccountsTrie, Inherent, InherentType, StakingContract};

/// We need to distinguish between two types of transactions:
/// 1. Incoming transactions, which include:
///     - Validator
///         * Create
///         * Update
///         * Retire
///         * Reactivate
///         * Unpark
///     - Staker
///         * Create
///         * Stake
///         * Update
///         * Retire
///         * Reactivate
///     The type of transaction is given in the data field.
/// 2. Outgoing transactions, which include:
///     - Validator
///         * Drop
///     - Staker
///         * Unstake
///         * Deduct fees
///     The type of transaction is given in the proof field.
impl AccountTransactionInteraction for StakingContract {
    fn create(
        _accounts_tree: &AccountsTrie,
        _db_txn: &mut WriteTransaction,
        _transaction: &Transaction,
        _block_height: u32,
        _block_time: u64,
    ) -> Result<(), AccountError> {
        Err(AccountError::InvalidForRecipient)
    }

    /// Commits an incoming transaction to the accounts trie.
    fn commit_incoming_transaction(
        accounts_tree: &AccountsTrie,
        db_txn: &mut WriteTransaction,
        transaction: &Transaction,
        block_height: u32,
        _block_time: u64,
    ) -> Result<Option<Vec<u8>>, AccountError> {
        // Check that the address is that of the Staking contract.
        if transaction.recipient != Address::from_any_str(policy::STAKING_CONTRACT_ADDRESS).unwrap()
        {
            return Err(AccountError::InvalidForRecipient);
        }

        let mut receipt = None;

        // Parse transaction data.
        let data = IncomingStakingTransactionData::parse(transaction)?;

        match data {
            IncomingStakingTransactionData::CreateValidator {
                warm_key,
                validator_key,
                reward_address,
                signal_data,
                proof,
                ..
            } => {
                // Get the validator address from the proof.
                let validator_address = proof.compute_signer();

                StakingContract::create_validator(
                    accounts_tree,
                    db_txn,
                    &validator_address,
                    warm_key,
                    validator_key,
                    reward_address,
                    signal_data,
                )?;
            }
            IncomingStakingTransactionData::UpdateValidator {
                new_warm_key,
                new_validator_key,
                new_reward_address,
                new_signal_data,
                proof,
                ..
            } => {
                // Get the validator address from the proof.
                let validator_address = proof.compute_signer();

                receipt = Some(
                    StakingContract::update_validator(
                        accounts_tree,
                        db_txn,
                        &validator_address,
                        new_warm_key,
                        new_validator_key,
                        new_reward_address,
                        new_signal_data,
                    )?
                    .serialize_to_vec(),
                )
            }
            IncomingStakingTransactionData::RetireValidator {
                validator_address,
                proof,
            } => {
                // Get the warm address from the proof.
                let warm_address = proof.compute_signer();

                receipt = Some(
                    StakingContract::retire_validator(
                        accounts_tree,
                        db_txn,
                        &validator_address,
                        warm_address,
                        block_height,
                    )?
                    .serialize_to_vec(),
                );
            }
            IncomingStakingTransactionData::ReactivateValidator {
                validator_address,
                proof,
            } => {
                // Get the warm address from the proof.
                let warm_address = proof.compute_signer();

                receipt = Some(
                    StakingContract::reactivate_validator(
                        accounts_tree,
                        db_txn,
                        &validator_address,
                        warm_address,
                    )?
                    .serialize_to_vec(),
                );
            }
            IncomingStakingTransactionData::UnparkValidator {
                validator_address,
                proof,
            } => {
                // Get the warm address from the proof.
                let warm_address = proof.compute_signer();

                receipt = Some(
                    StakingContract::unpark_validator(
                        accounts_tree,
                        db_txn,
                        &validator_address,
                        warm_address,
                    )?
                    .serialize_to_vec(),
                );
            }
            IncomingStakingTransactionData::CreateStaker { delegation, proof } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                StakingContract::create_staker(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    transaction.value,
                    delegation,
                )?;
            }
            IncomingStakingTransactionData::Stake { staker_address } => {
                StakingContract::stake(accounts_tree, db_txn, &staker_address, transaction.value)?;
            }
            IncomingStakingTransactionData::UpdateStaker {
                new_delegation,
                proof,
            } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                receipt = Some(
                    StakingContract::update_staker(
                        accounts_tree,
                        db_txn,
                        &staker_address,
                        new_delegation,
                    )?
                    .serialize_to_vec(),
                );
            }
            IncomingStakingTransactionData::RetireStaker { value, proof } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                receipt = Some(
                    StakingContract::retire_staker(
                        accounts_tree,
                        db_txn,
                        &staker_address,
                        value,
                        block_height,
                    )?
                    .serialize_to_vec(),
                );
            }
            IncomingStakingTransactionData::ReactivateStaker { value, proof } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                StakingContract::reactivate_staker(accounts_tree, db_txn, &staker_address, value)?;
            }
        }

        Ok(receipt)
    }

    /// Reverts the commit of an incoming transaction to the accounts trie.
    fn revert_incoming_transaction(
        accounts_tree: &AccountsTrie,
        db_txn: &mut WriteTransaction,
        transaction: &Transaction,
        _block_height: u32,
        _time: u64,
        receipt: Option<&Vec<u8>>,
    ) -> Result<(), AccountError> {
        // Parse transaction data.
        let data = IncomingStakingTransactionData::parse(transaction)?;

        match data {
            IncomingStakingTransactionData::CreateValidator { proof, .. } => {
                // Get the validator address from the proof.
                let validator_address = proof.compute_signer();

                StakingContract::revert_create_validator(
                    accounts_tree,
                    db_txn,
                    &validator_address,
                )?;
            }
            IncomingStakingTransactionData::UpdateValidator { proof, .. } => {
                // Get the validator address from the proof.
                let validator_address = proof.compute_signer();

                let receipt = Deserialize::deserialize_from_vec(
                    receipt.ok_or(AccountError::InvalidReceipt)?,
                )?;

                StakingContract::revert_update_validator(
                    accounts_tree,
                    db_txn,
                    &validator_address,
                    receipt,
                )?;
            }
            IncomingStakingTransactionData::RetireValidator {
                validator_address, ..
            } => {
                let receipt = Deserialize::deserialize_from_vec(
                    receipt.ok_or(AccountError::InvalidReceipt)?,
                )?;

                StakingContract::revert_retire_validator(
                    accounts_tree,
                    db_txn,
                    &validator_address,
                    receipt,
                )?;
            }
            IncomingStakingTransactionData::ReactivateValidator {
                validator_address, ..
            } => {
                let receipt = Deserialize::deserialize_from_vec(
                    receipt.ok_or(AccountError::InvalidReceipt)?,
                )?;

                StakingContract::revert_reactivate_validator(
                    accounts_tree,
                    db_txn,
                    &validator_address,
                    receipt,
                )?;
            }
            IncomingStakingTransactionData::UnparkValidator {
                validator_address, ..
            } => {
                let receipt = Deserialize::deserialize_from_vec(
                    receipt.ok_or(AccountError::InvalidReceipt)?,
                )?;

                StakingContract::revert_unpark_validator(
                    accounts_tree,
                    db_txn,
                    &validator_address,
                    receipt,
                )?;
            }
            IncomingStakingTransactionData::CreateStaker { proof, .. } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                StakingContract::revert_create_staker(accounts_tree, db_txn, &staker_address)?;
            }
            IncomingStakingTransactionData::Stake { staker_address } => {
                StakingContract::revert_stake(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    transaction.value,
                )?;
            }
            IncomingStakingTransactionData::UpdateStaker { proof, .. } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                let receipt = Deserialize::deserialize_from_vec(
                    receipt.ok_or(AccountError::InvalidReceipt)?,
                )?;

                StakingContract::revert_update_staker(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    receipt,
                )?;
            }
            IncomingStakingTransactionData::RetireStaker { value, proof } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                let receipt = Deserialize::deserialize_from_vec(
                    receipt.ok_or(AccountError::InvalidReceipt)?,
                )?;

                StakingContract::revert_retire_staker(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    value,
                    receipt,
                )?;
            }
            IncomingStakingTransactionData::ReactivateStaker { value, proof } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                StakingContract::revert_reactivate_staker(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    value,
                )?;
            }
        }

        Ok(())
    }

    /// Commits an outgoing transaction to the accounts trie.
    fn commit_outgoing_transaction(
        accounts_tree: &AccountsTrie,
        db_txn: &mut WriteTransaction,
        transaction: &Transaction,
        block_height: u32,
        _block_time: u64,
    ) -> Result<Option<Vec<u8>>, AccountError> {
        // Check that the address is that of the Staking contract.
        if transaction.sender != Address::from_any_str(policy::STAKING_CONTRACT_ADDRESS).unwrap() {
            return Err(AccountError::InvalidForSender);
        }

        let receipt;

        // Parse transaction data.
        let data = OutgoingStakingTransactionProof::parse(transaction)?;

        match data {
            OutgoingStakingTransactionProof::DropValidator { proof } => {
                // Get the validator address from the proof.
                let validator_address = proof.compute_signer();

                receipt = Some(
                    StakingContract::drop_validator(
                        accounts_tree,
                        db_txn,
                        &validator_address,
                        block_height,
                    )?
                    .serialize_to_vec(),
                );
            }
            OutgoingStakingTransactionProof::Unstake { proof } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                receipt = StakingContract::unstake(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    transaction.total_value()?,
                    block_height,
                )?
                .map(|r| r.serialize_to_vec());
            }
            OutgoingStakingTransactionProof::DeductFees {
                from_active_balance,
                proof,
            } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                receipt = StakingContract::deduct_fees(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    from_active_balance,
                    transaction.fee,
                )?
                .map(|r| r.serialize_to_vec());
            }
        }

        Ok(receipt)
    }

    /// Reverts the commit of an incoming transaction to the accounts trie.
    fn revert_outgoing_transaction(
        accounts_tree: &AccountsTrie,
        db_txn: &mut WriteTransaction,
        transaction: &Transaction,
        _block_height: u32,
        _block_time: u64,
        receipt: Option<&Vec<u8>>,
    ) -> Result<(), AccountError> {
        // Parse transaction data.
        let data = OutgoingStakingTransactionProof::parse(transaction)?;

        match data {
            OutgoingStakingTransactionProof::DropValidator { proof } => {
                // Get the validator address from the proof.
                let validator_address = proof.compute_signer();

                let receipt: DropValidatorReceipt = Deserialize::deserialize_from_vec(
                    receipt.ok_or(AccountError::InvalidReceipt)?,
                )?;

                StakingContract::revert_drop_validator(
                    accounts_tree,
                    db_txn,
                    &validator_address,
                    receipt,
                )?;
            }
            OutgoingStakingTransactionProof::Unstake { proof } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                let receipt = match receipt {
                    Some(v) => Some(Deserialize::deserialize_from_vec(v)?),
                    None => None,
                };

                StakingContract::revert_unstake(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    transaction.total_value()?,
                    receipt,
                )?;
            }
            OutgoingStakingTransactionProof::DeductFees {
                from_active_balance,
                proof,
            } => {
                // Get the staker address from the proof.
                let staker_address = proof.compute_signer();

                let receipt = match receipt {
                    Some(v) => Some(Deserialize::deserialize_from_vec(v)?),
                    None => None,
                };

                StakingContract::revert_deduct_fees(
                    accounts_tree,
                    db_txn,
                    &staker_address,
                    from_active_balance,
                    transaction.fee,
                    receipt,
                )?;
            }
        }

        Ok(())
    }
}

impl AccountInherentInteraction for StakingContract {
    /// Commits an inherent to the accounts trie.
    fn commit_inherent(
        accounts_tree: &AccountsTrie,
        db_txn: &mut WriteTransaction,
        inherent: &Inherent,
        block_height: u32,
        _block_time: u64,
    ) -> Result<Option<Vec<u8>>, AccountError> {
        trace!("Committing inherent to accounts trie: {:?}", inherent);

        // None of the allowed inherents for the staking contract has a value. Only reward inherents
        // have a value.
        if inherent.value != Coin::ZERO {
            return Err(AccountError::InvalidInherent);
        }

        // Get the staking contract.
        let mut staking_contract = StakingContract::get_staking_contract(accounts_tree, db_txn);

        let receipt;

        match &inherent.ty {
            InherentType::Slash => {
                // Check data length.
                if inherent.data.len() != SlashedSlot::SIZE {
                    return Err(AccountError::InvalidInherent);
                }

                // Deserialize slot.
                let slot: SlashedSlot = Deserialize::deserialize(&mut &inherent.data[..])?;

                // Check that the slashed validator does exist.
                if StakingContract::get_validator(accounts_tree, db_txn, &slot.validator_address)
                    .is_none()
                {
                    return Err(AccountError::InvalidInherent);
                }

                // Add the validator address to the parked set.
                // TODO: The inherent might have originated from a fork proof for the previous epoch.
                //  Right now, we don't care and start the parking period in the epoch the proof has been submitted.
                let newly_parked = staking_contract
                    .parked_set
                    .insert(slot.validator_address.clone());

                // Fork proof from previous epoch should affect:
                // - previous_lost_rewards
                // - previous_disabled_slots (not needed, because it's redundant with the lost rewards)
                // Fork proof from current epoch, but previous batch should affect:
                // - previous_lost_rewards
                // - current_disabled_slots
                // All others:
                // - current_lost_rewards
                // - current_disabled_slots
                let newly_disabled;
                let newly_lost_rewards;

                if policy::epoch_at(slot.event_block) < policy::epoch_at(block_height) {
                    newly_lost_rewards = !staking_contract
                        .previous_lost_rewards
                        .contains(slot.slot as usize);

                    staking_contract
                        .previous_lost_rewards
                        .insert(slot.slot as usize);

                    newly_disabled = false;
                } else if policy::batch_at(slot.event_block) < policy::batch_at(block_height) {
                    newly_lost_rewards = !staking_contract
                        .previous_lost_rewards
                        .contains(slot.slot as usize);

                    staking_contract
                        .previous_lost_rewards
                        .insert(slot.slot as usize);

                    newly_disabled = staking_contract
                        .current_disabled_slots
                        .entry(slot.validator_address.clone())
                        .or_insert_with(BTreeSet::new)
                        .insert(slot.slot);
                } else {
                    newly_lost_rewards = !staking_contract
                        .current_lost_rewards
                        .contains(slot.slot as usize);

                    staking_contract
                        .current_lost_rewards
                        .insert(slot.slot as usize);

                    newly_disabled = staking_contract
                        .current_disabled_slots
                        .entry(slot.validator_address.clone())
                        .or_insert_with(BTreeSet::new)
                        .insert(slot.slot);
                }

                receipt = Some(
                    SlashReceipt {
                        newly_parked,
                        newly_disabled,
                        newly_lost_rewards,
                    }
                    .serialize_to_vec(),
                );
            }
            InherentType::FinalizeBatch | InherentType::FinalizeEpoch => {
                // Invalid data length
                if !inherent.data.is_empty() {
                    return Err(AccountError::InvalidInherent);
                }

                // Clear the lost rewards set.
                staking_contract.previous_lost_rewards = staking_contract.current_lost_rewards;
                staking_contract.current_lost_rewards = BitSet::new();

                // Parking set and disabled slots are only cleared on epoch changes.
                if inherent.ty == InherentType::FinalizeEpoch {
                    // But first, retire all validators that have been parked this epoch.
                    for validator_address in staking_contract.parked_set {
                        // Get the validator and update it.
                        let mut validator = StakingContract::get_validator(
                            accounts_tree,
                            db_txn,
                            &validator_address,
                        )
                        .ok_or(AccountError::InvalidInherent)?;

                        validator.inactivity_flag = Some(block_height);

                        trace!(
                            "Trying to put validator with address {} in the accounts tree.",
                            validator_address.to_string(),
                        );

                        accounts_tree.put(
                            db_txn,
                            &StakingContract::get_key_validator(&validator_address),
                            Account::StakingValidator(validator),
                        );

                        // Update the staking contract.
                        staking_contract
                            .active_validators
                            .remove(&validator_address);
                    }

                    // Now we clear the parking set.
                    staking_contract.parked_set = BTreeSet::new();

                    // And the disabled slots.
                    // Optimization: We actually only need the old slots for the first batch of the epoch.
                    staking_contract.previous_disabled_slots =
                        staking_contract.current_disabled_slots;
                    staking_contract.current_disabled_slots = BTreeMap::new();
                }

                // Since finalized epochs cannot be reverted, we don't need any receipts.
                receipt = None;
            }
            InherentType::Reward => {
                return Err(AccountError::InvalidForTarget);
            }
        }

        trace!("Trying to put the staking contract in the accounts tree.");

        accounts_tree.put(
            db_txn,
            &StakingContract::get_key_staking_contract(),
            Account::Staking(staking_contract),
        );

        Ok(receipt)
    }

    /// Reverts the commit of an inherent to the accounts trie.
    fn revert_inherent(
        accounts_tree: &AccountsTrie,
        db_txn: &mut WriteTransaction,
        inherent: &Inherent,
        block_height: u32,
        _block_time: u64,
        receipt: Option<&Vec<u8>>,
    ) -> Result<(), AccountError> {
        // Get the staking contract main.
        let mut staking_contract = StakingContract::get_staking_contract(accounts_tree, db_txn);

        match &inherent.ty {
            InherentType::Slash => {
                let receipt: SlashReceipt = Deserialize::deserialize_from_vec(
                    receipt.ok_or(AccountError::InvalidReceipt)?,
                )?;

                let slot: SlashedSlot = Deserialize::deserialize(&mut &inherent.data[..])?;

                // Only remove if it was not already slashed.
                // I kept this in two nested if's for clarity.
                if receipt.newly_parked {
                    let has_been_removed =
                        staking_contract.parked_set.remove(&slot.validator_address);
                    if !has_been_removed {
                        return Err(AccountError::InvalidInherent);
                    }
                }

                // Fork proof from previous epoch should affect:
                // - previous_lost_rewards
                // - previous_disabled_slots (not needed, because it's redundant with the lost rewards)
                // Fork proof from current epoch, but previous batch should affect:
                // - previous_lost_rewards
                // - current_disabled_slots
                // All others:
                // - current_lost_rewards
                // - current_disabled_slots
                if receipt.newly_disabled {
                    if policy::epoch_at(slot.event_block) < policy::epoch_at(block_height) {
                        // Nothing to do.
                    } else {
                        let is_empty = {
                            let entry = staking_contract
                                .current_disabled_slots
                                .get_mut(&slot.validator_address)
                                .unwrap();
                            entry.remove(&slot.slot);
                            entry.is_empty()
                        };
                        if is_empty {
                            staking_contract
                                .current_disabled_slots
                                .remove(&slot.validator_address);
                        }
                    }
                }
                if receipt.newly_lost_rewards {
                    if policy::epoch_at(slot.event_block) < policy::epoch_at(block_height)
                        || policy::batch_at(slot.event_block) < policy::batch_at(block_height)
                    {
                        staking_contract
                            .previous_lost_rewards
                            .remove(slot.slot as usize);
                    } else {
                        staking_contract
                            .current_lost_rewards
                            .remove(slot.slot as usize);
                    }
                }
            }
            InherentType::FinalizeBatch | InherentType::FinalizeEpoch => {
                // We should not be able to revert finalized epochs or batches!
                return Err(AccountError::InvalidForTarget);
            }
            InherentType::Reward => {
                return Err(AccountError::InvalidForTarget);
            }
        }

        trace!("Trying to put the staking contract in the accounts tree.");

        accounts_tree.put(
            db_txn,
            &StakingContract::get_key_staking_contract(),
            Account::Staking(staking_contract),
        );

        Ok(())
    }
}
