use super::*;
use alloc::string::String;
use frame_support::pallet_prelude::Weight;
use sp_io::KillStorageResult;
use sp_io::hashing::twox_128;
use sp_io::storage::clear_prefix;
pub mod migrate_chain_identity;
pub mod migrate_coldkey_swap_scheduled;
pub mod migrate_commit_reveal_v2;
pub mod migrate_create_root_network;
pub mod migrate_crv3_commits_add_block;
pub mod migrate_delete_subnet_21;
pub mod migrate_delete_subnet_3;
pub mod migrate_fix_is_network_member;
pub mod migrate_fix_root_subnet_tao;
pub mod migrate_identities_v2;
pub mod migrate_init_total_issuance;
pub mod migrate_orphaned_storage_items;
pub mod migrate_populate_owned_hotkeys;
pub mod migrate_rao;
pub mod migrate_remove_commitments_rate_limit;
pub mod migrate_remove_stake_map;
pub mod migrate_remove_total_hotkey_coldkey_stakes_this_interval;
pub mod migrate_remove_unused_maps_and_values;
pub mod migrate_remove_zero_total_hotkey_alpha;
pub mod migrate_reset_bonds_moving_average;
pub mod migrate_reset_max_burn;
pub mod migrate_set_first_emission_block_number;
pub mod migrate_set_min_burn;
pub mod migrate_set_min_difficulty;
pub mod migrate_set_nominator_min_stake;
pub mod migrate_set_registration_enable;
pub mod migrate_set_subtoken_enabled;
pub mod migrate_stake_threshold;
pub mod migrate_subnet_identities_to_v3;
pub mod migrate_subnet_symbols;
pub mod migrate_subnet_volume;
pub mod migrate_to_v1_separate_emission;
pub mod migrate_to_v2_fixed_total_stake;
pub mod migrate_total_issuance;
pub mod migrate_transfer_ownership_to_foundation;
pub mod migrate_upgrade_revealed_commitments;

pub(crate) fn migrate_storage<T: Config>(
    migration_name: &'static str,
    pallet_name: &'static str,
    storage_name: &'static str,
) -> Weight {
    let migration_name_bytes = migration_name.as_bytes().to_vec();

    let mut weight = T::DbWeight::get().reads(1);
    if HasMigrationRun::<T>::get(&migration_name_bytes) {
        log::info!(
            "Migration '{:?}' has already run. Skipping.",
            String::from_utf8_lossy(&migration_name_bytes)
        );
        return weight;
    }

    log::info!("Running migration '{}'", migration_name);

    let pallet_name = twox_128(pallet_name.as_bytes());
    let storage_name = twox_128(storage_name.as_bytes());
    let prefix = [pallet_name, storage_name].concat();

    // Remove all entries.
    let removed_entries_count = match clear_prefix(&prefix, Some(u32::MAX)) {
        KillStorageResult::AllRemoved(removed) => {
            log::info!("Removed all entries from {:?}.", storage_name);

            // Mark migration as completed
            HasMigrationRun::<T>::insert(&migration_name_bytes, true);
            weight = weight.saturating_add(T::DbWeight::get().writes(1));

            removed as u64
        }
        KillStorageResult::SomeRemaining(removed) => {
            log::info!("Failed to remove all entries from {:?}", storage_name);
            removed as u64
        }
    };

    weight = weight.saturating_add(T::DbWeight::get().writes(removed_entries_count as u64));

    log::info!(
        "Migration '{:?}' completed successfully. {:?} entries removed.",
        migration_name,
        removed_entries_count
    );

    weight
}
