use super::*;
use frame_support::{traits::Get, weights::Weight};
use log;
use scale_info::prelude::string::String;
use sp_std::collections::vec_deque::VecDeque;

/// --------------- Migration ------------------------------------------
/// Upgrades every entry to the new 4-tuple layout by inserting
/// `commit_block = first_block_of_epoch(netuid, epoch)`.
pub fn migrate_crv3_commits_add_block<T: Config>() -> Weight {
    let mig_name: Vec<u8> = b"crv3_commits_add_block_v1".to_vec();
    let mut total_weight = T::DbWeight::get().reads(1);

    // run once
    if HasMigrationRun::<T>::get(&mig_name) {
        log::info!(
            "Migration '{}' already executed - skipping",
            String::from_utf8_lossy(&mig_name)
        );
        return total_weight;
    }
    log::info!("Running migration '{}'", String::from_utf8_lossy(&mig_name));

    // iterate over *all* (netuid, epoch, queue) triples
    for (netuid, epoch, old_q) in CRV3WeightCommits::<T>::drain() {
        total_weight = total_weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));

        let commit_block = Pallet::<T>::get_first_block_of_epoch(netuid, epoch);

        // convert VecDeque<(who,cipher,rnd)> → VecDeque<(who,cb,cipher,rnd)>
        let new_q: VecDeque<_> = old_q
            .into_iter()
            .map(|(who, cipher, rnd)| (who, commit_block, cipher, rnd))
            .collect();

        // write back under *new* storage definition
        CRV3WeightCommitsV2::<T>::insert(netuid, epoch, new_q);
    }

    // mark as done
    HasMigrationRun::<T>::insert(&mig_name, true);
    total_weight = total_weight.saturating_add(T::DbWeight::get().writes(1));

    log::info!(
        "Migration '{}' completed",
        String::from_utf8_lossy(&mig_name)
    );
    total_weight
}
