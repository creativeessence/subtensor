#![allow(
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::indexing_slicing
)]
use super::mock::*;
use crate::{subnets::leasing::SubnetLeaseOf, *};
use frame_support::{StorageDoubleMap, assert_err, assert_ok};
use sp_core::U256;
use sp_runtime::Percent;
use substrate_fixed::types::U64F64;
use subtensor_runtime_common::AlphaCurrency;

#[test]
fn test_register_leased_network_works() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Register the leased network
        let end_block = 500;
        let emissions_share = Percent::from_percent(30);
        assert_ok!(SubtensorModule::register_leased_network(
            RuntimeOrigin::signed(beneficiary),
            emissions_share,
            Some(end_block),
        ));

        // Ensure the lease was created
        let lease_id = 0;
        let lease = SubnetLeases::<Test>::get(lease_id).unwrap();
        assert_eq!(lease.beneficiary, beneficiary);
        assert_eq!(lease.emissions_share, emissions_share);
        assert_eq!(lease.end_block, Some(end_block));

        // Ensure the subnet exists
        assert!(SubnetMechanism::<Test>::contains_key(lease.netuid));

        // Ensure the subnet uid to lease id mapping exists
        assert_eq!(
            SubnetUidToLeaseId::<Test>::get(lease.netuid),
            Some(lease_id)
        );

        // Ensure the beneficiary has been added as a proxy
        assert!(PROXIES.with_borrow(|proxies| proxies.0 == vec![(lease.coldkey, beneficiary)]));

        // Ensure the lease shares have been created for each contributor
        let contributor1_share = U64F64::from(contributions[0].1).saturating_div(U64F64::from(cap));
        assert_eq!(
            SubnetLeaseShares::<Test>::get(lease_id, contributions[0].0),
            contributor1_share
        );
        let contributor2_share = U64F64::from(contributions[1].1).saturating_div(U64F64::from(cap));
        assert_eq!(
            SubnetLeaseShares::<Test>::get(lease_id, contributions[1].0),
            contributor2_share
        );

        // Ensure each contributor and beneficiary has been refunded their share of the leftover cap
        let leftover_cap = cap.saturating_sub(lease.cost);

        let expected_contributor1_refund = U64F64::from(leftover_cap)
            .saturating_mul(contributor1_share)
            .floor()
            .to_num::<u64>();
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[0].0),
            expected_contributor1_refund
        );

        let expected_contributor2_refund = U64F64::from(leftover_cap)
            .saturating_mul(contributor2_share)
            .floor()
            .to_num::<u64>();
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[1].0),
            expected_contributor2_refund
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&beneficiary),
            leftover_cap - (expected_contributor1_refund + expected_contributor2_refund)
        );

        // Ensure the event is emitted
        assert_eq!(
            last_event(),
            crate::Event::<Test>::SubnetLeaseCreated {
                beneficiary,
                lease_id,
                netuid: lease.netuid,
                end_block: Some(end_block),
            }
            .into()
        );
    });
}

#[test]
fn test_register_leased_network_fails_if_bad_origin() {
    new_test_ext(1).execute_with(|| {
        let end_block = 500;
        let emissions_share = Percent::from_percent(30);

        assert_err!(
            SubtensorModule::register_leased_network(
                RuntimeOrigin::none(),
                emissions_share,
                Some(end_block),
            ),
            DispatchError::BadOrigin,
        );

        assert_err!(
            SubtensorModule::register_leased_network(
                RuntimeOrigin::root(),
                emissions_share,
                Some(end_block),
            ),
            DispatchError::BadOrigin,
        );
    });
}

#[test]
fn test_register_leased_network_fails_if_crowdloan_does_not_exists() {
    new_test_ext(1).execute_with(|| {
        let beneficiary = U256::from(1);
        let end_block = 500;
        let emissions_share = Percent::from_percent(30);

        assert_err!(
            SubtensorModule::register_leased_network(
                RuntimeOrigin::signed(beneficiary),
                emissions_share,
                Some(end_block),
            ),
            pallet_crowdloan::Error::<Test>::InvalidCrowdloanId,
        );
    });
}

#[test]
fn test_register_lease_network_fails_if_current_crowdloan_id_is_not_set() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Mark as if the current crowdloan id is not set
        pallet_crowdloan::CurrentCrowdloanId::<Test>::set(None);

        let end_block = 500;
        let emissions_share = Percent::from_percent(30);

        assert_err!(
            SubtensorModule::register_leased_network(
                RuntimeOrigin::signed(beneficiary),
                emissions_share,
                Some(end_block),
            ),
            pallet_crowdloan::Error::<Test>::InvalidCrowdloanId,
        );
    });
}

#[test]
fn test_register_leased_network_fails_if_origin_is_not_crowdloan_creator() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        let end_block = 500;
        let emissions_share = Percent::from_percent(30);

        assert_err!(
            SubtensorModule::register_leased_network(
                RuntimeOrigin::signed(U256::from(2)),
                emissions_share,
                Some(end_block),
            ),
            Error::<Test>::InvalidLeaseBeneficiary,
        );
    });
}

#[test]
fn test_register_lease_network_fails_if_end_block_is_in_the_past() {
    new_test_ext(501).execute_with(|| {
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        let end_block = 500;
        let emissions_share = Percent::from_percent(30);

        assert_err!(
            SubtensorModule::register_leased_network(
                RuntimeOrigin::signed(beneficiary),
                emissions_share,
                Some(end_block),
            ),
            Error::<Test>::LeaseCannotEndInThePast,
        );
    });
}

#[test]
fn test_terminate_lease_works() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![(U256::from(2), 990_000_000_000)]; // 990 TAO
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let emissions_share = Percent::from_percent(30);
        let (lease_id, lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            Some(tao_to_stake),
        );

        // Run to the end of the lease
        run_to_block(end_block);

        // Create a hotkey for the beneficiary
        let hotkey = U256::from(3);
        SubtensorModule::create_account_if_non_existent(&beneficiary, &hotkey);

        // Terminate the lease
        assert_ok!(SubtensorModule::terminate_lease(
            RuntimeOrigin::signed(beneficiary),
            lease_id,
            hotkey,
        ));

        // Ensure the beneficiary is now the owner of the subnet
        assert_eq!(SubnetOwner::<Test>::get(lease.netuid), beneficiary);
        assert_eq!(SubnetOwnerHotkey::<Test>::get(lease.netuid), hotkey);

        // Ensure everything has been cleaned up
        assert_eq!(SubnetLeases::<Test>::get(lease_id), None);
        assert!(!SubnetLeaseShares::<Test>::contains_prefix(lease_id));
        assert!(!AccumulatedLeaseDividends::<Test>::contains_key(lease_id));

        // Ensure the beneficiary has been removed as a proxy
        assert!(PROXIES.with_borrow(|proxies| proxies.0.is_empty()));

        // Ensure the event is emitted
        assert_eq!(
            last_event(),
            crate::Event::<Test>::SubnetLeaseTerminated {
                beneficiary: lease.beneficiary,
                netuid: lease.netuid,
            }
            .into()
        );
    });
}

#[test]
fn test_terminate_lease_fails_if_bad_origin() {
    new_test_ext(1).execute_with(|| {
        let lease_id = 0;
        let hotkey = U256::from(1);

        assert_err!(
            SubtensorModule::terminate_lease(RuntimeOrigin::none(), lease_id, hotkey),
            DispatchError::BadOrigin,
        );

        assert_err!(
            SubtensorModule::terminate_lease(RuntimeOrigin::root(), lease_id, hotkey),
            DispatchError::BadOrigin,
        );
    });
}

#[test]
fn test_terminate_lease_fails_if_lease_does_not_exist() {
    new_test_ext(1).execute_with(|| {
        let lease_id = 0;
        let beneficiary = U256::from(1);
        let hotkey = U256::from(2);

        assert_err!(
            SubtensorModule::terminate_lease(RuntimeOrigin::signed(beneficiary), lease_id, hotkey),
            Error::<Test>::LeaseDoesNotExist,
        );
    });
}

#[test]
fn test_terminate_lease_fails_if_origin_is_not_beneficiary() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![(U256::from(2), 990_000_000_000)]; // 990 TAO
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let emissions_share = Percent::from_percent(30);
        let (lease_id, _lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            Some(tao_to_stake),
        );

        // Run to the end of the lease
        run_to_block(end_block);

        // Create a hotkey for the beneficiary
        let hotkey = U256::from(3);
        SubtensorModule::create_account_if_non_existent(&beneficiary, &hotkey);

        // Terminate the lease
        assert_err!(
            SubtensorModule::terminate_lease(
                RuntimeOrigin::signed(U256::from(42)),
                lease_id,
                hotkey,
            ),
            Error::<Test>::ExpectedBeneficiaryOrigin,
        );
    });
}

#[test]
fn test_terminate_lease_fails_if_lease_has_no_end_block() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![(U256::from(2), 990_000_000_000)]; // 990 TAO
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let emissions_share = Percent::from_percent(30);
        let (lease_id, lease) =
            setup_leased_network(beneficiary, emissions_share, None, Some(tao_to_stake));

        // Create a hotkey for the beneficiary
        let hotkey = U256::from(3);
        SubtensorModule::create_account_if_non_existent(&beneficiary, &hotkey);

        // Terminate the lease
        assert_err!(
            SubtensorModule::terminate_lease(
                RuntimeOrigin::signed(lease.beneficiary),
                lease_id,
                hotkey,
            ),
            Error::<Test>::LeaseHasNoEndBlock,
        );
    });
}

#[test]
fn test_terminate_lease_fails_if_lease_has_not_ended() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![(U256::from(2), 990_000_000_000)]; // 990 TAO
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let emissions_share = Percent::from_percent(30);
        let (lease_id, lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            Some(tao_to_stake),
        );

        // Create a hotkey for the beneficiary
        let hotkey = U256::from(3);
        SubtensorModule::create_account_if_non_existent(&beneficiary, &hotkey);

        // Terminate the lease
        assert_err!(
            SubtensorModule::terminate_lease(
                RuntimeOrigin::signed(lease.beneficiary),
                lease_id,
                hotkey,
            ),
            Error::<Test>::LeaseHasNotEnded,
        );
    });
}

#[test]
fn test_terminate_lease_fails_if_beneficiary_does_not_own_hotkey() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![(U256::from(2), 990_000_000_000)]; // 990 TAO
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let emissions_share = Percent::from_percent(30);
        let (lease_id, lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            Some(tao_to_stake),
        );

        // Run to the end of the lease
        run_to_block(end_block);

        // Terminate the lease
        assert_err!(
            SubtensorModule::terminate_lease(
                RuntimeOrigin::signed(lease.beneficiary),
                lease_id,
                U256::from(42),
            ),
            Error::<Test>::BeneficiaryDoesNotOwnHotkey,
        );
    });
}
#[test]
fn test_distribute_lease_network_dividends_multiple_contributors_works() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let emissions_share = Percent::from_percent(30);
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let (lease_id, lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            Some(tao_to_stake),
        );

        // Setup the correct block to distribute dividends
        run_to_block(<Test as Config>::LeaseDividendsDistributionInterval::get() as u64);

        // Get the initial subnet tao after stake and ensure all contributor
        // balances are in initial state
        let subnet_tao_before = SubnetTAO::<Test>::get(lease.netuid);
        let contributor1_balance_before = SubtensorModule::get_coldkey_balance(&contributions[0].0);
        let contributor2_balance_before = SubtensorModule::get_coldkey_balance(&contributions[1].0);
        let beneficiary_balance_before = SubtensorModule::get_coldkey_balance(&beneficiary);

        // Setup some previously accumulated dividends
        let accumulated_dividends = AlphaCurrency::from(5_000_000);
        AccumulatedLeaseDividends::<Test>::insert(lease_id, accumulated_dividends);

        // Distribute the dividends
        let owner_cut_alpha = AlphaCurrency::from(5_000_000);
        SubtensorModule::distribute_leased_network_dividends(lease_id, owner_cut_alpha);

        // Ensure the dividends were distributed correctly relative to their shares
        let distributed_tao = subnet_tao_before - SubnetTAO::<Test>::get(lease.netuid);
        let contributor1_balance_delta = SubtensorModule::get_coldkey_balance(&contributions[0].0)
            .saturating_sub(contributor1_balance_before);
        let contributor2_balance_delta = SubtensorModule::get_coldkey_balance(&contributions[1].0)
            .saturating_sub(contributor2_balance_before);
        let beneficiary_balance_delta = SubtensorModule::get_coldkey_balance(&beneficiary)
            .saturating_sub(beneficiary_balance_before);

        assert_eq!(
            distributed_tao,
            beneficiary_balance_delta + contributor1_balance_delta + contributor2_balance_delta
        );

        let expected_contributor1_balance =
            SubnetLeaseShares::<Test>::get(lease_id, contributions[0].0)
                .saturating_mul(U64F64::from(distributed_tao))
                .floor()
                .to_num::<u64>();
        assert_eq!(contributor1_balance_delta, expected_contributor1_balance);

        let expected_contributor2_balance =
            SubnetLeaseShares::<Test>::get(lease_id, contributions[1].0)
                .saturating_mul(U64F64::from(distributed_tao))
                .floor()
                .to_num::<u64>();
        assert_eq!(contributor2_balance_delta, expected_contributor2_balance);

        // The beneficiary should have received the remaining dividends
        let expected_beneficiary_balance =
            distributed_tao - (expected_contributor1_balance + expected_contributor2_balance);
        assert_eq!(beneficiary_balance_delta, expected_beneficiary_balance);

        // Ensure nothing was accumulated for later distribution
        assert_eq!(
            AccumulatedLeaseDividends::<Test>::get(lease_id),
            AlphaCurrency::ZERO
        );
    });
}

#[test]
fn test_distribute_lease_network_dividends_only_beneficiary_works() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![(U256::from(1), 990_000_000_000)]; // 990 TAO
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let emissions_share = Percent::from_percent(30);
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let (lease_id, lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            Some(tao_to_stake),
        );

        // Setup the correct block to distribute dividends
        run_to_block(<Test as Config>::LeaseDividendsDistributionInterval::get() as u64);

        // Get the initial subnet tao after stake and beneficiary balance
        let subnet_tao_before = SubnetTAO::<Test>::get(lease.netuid);
        let beneficiary_balance_before = SubtensorModule::get_coldkey_balance(&beneficiary);

        // Setup some previously accumulated dividends
        let accumulated_dividends = AlphaCurrency::from(5_000_000);
        AccumulatedLeaseDividends::<Test>::insert(lease_id, accumulated_dividends);

        // Distribute the dividends
        let owner_cut_alpha = AlphaCurrency::from(5_000_000);
        SubtensorModule::distribute_leased_network_dividends(lease_id, owner_cut_alpha);

        // Ensure the dividends were distributed correctly relative to their shares
        let distributed_tao = subnet_tao_before - SubnetTAO::<Test>::get(lease.netuid);
        let beneficiary_balance_delta = SubtensorModule::get_coldkey_balance(&beneficiary)
            .saturating_sub(beneficiary_balance_before);
        assert_eq!(distributed_tao, beneficiary_balance_delta);

        // Ensure nothing was accumulated for later distribution
        assert_eq!(
            AccumulatedLeaseDividends::<Test>::get(lease_id),
            AlphaCurrency::ZERO
        );
    });
}

#[test]
fn test_distribute_lease_network_dividends_accumulates_if_not_the_correct_block() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let emissions_share = Percent::from_percent(30);
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let (lease_id, _) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            Some(tao_to_stake),
        );

        // Setup incorrect block to distribute dividends
        run_to_block(<Test as Config>::LeaseDividendsDistributionInterval::get() as u64 + 1);

        // Get the initial subnet tao after stake and ensure all contributor
        let contributor1_balance_before = SubtensorModule::get_coldkey_balance(&contributions[0].0);
        let contributor2_balance_before = SubtensorModule::get_coldkey_balance(&contributions[1].0);
        let beneficiary_balance_before = SubtensorModule::get_coldkey_balance(&beneficiary);

        // Setup some previously accumulated dividends
        let accumulated_dividends = AlphaCurrency::from(5_000_000);
        AccumulatedLeaseDividends::<Test>::insert(lease_id, accumulated_dividends);

        // Distribute the dividends
        let owner_cut_alpha = AlphaCurrency::from(5_000_000);
        SubtensorModule::distribute_leased_network_dividends(lease_id, owner_cut_alpha);

        // Ensure the dividends were not distributed
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[0].0),
            contributor1_balance_before
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[1].0),
            contributor2_balance_before
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&beneficiary),
            beneficiary_balance_before
        );

        // Ensure we correctly accumulated the dividends
        assert_eq!(
            AccumulatedLeaseDividends::<Test>::get(lease_id),
            (accumulated_dividends + emissions_share.mul_ceil(owner_cut_alpha.to_u64()).into())
                .into()
        );
    });
}

#[test]
fn test_distribute_lease_network_dividends_does_nothing_if_lease_does_not_exist() {
    new_test_ext(1).execute_with(|| {
        let lease_id = 0;
        let owner_cut_alpha = AlphaCurrency::from(5_000_000);
        SubtensorModule::distribute_leased_network_dividends(lease_id, owner_cut_alpha);
    });
}

#[test]
fn test_distribute_lease_network_dividends_does_nothing_if_lease_has_ended() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];
        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let tao_to_stake = 100_000_000_000; // 100 TAO
        let emissions_share = Percent::from_percent(30);
        let (lease_id, lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            Some(tao_to_stake),
        );

        // Run to the end of the lease
        run_to_block(end_block);

        let subnet_tao_before = SubnetTAO::<Test>::get(lease.netuid);
        let contributor1_balance_before = SubtensorModule::get_coldkey_balance(&contributions[0].0);
        let contributor2_balance_before = SubtensorModule::get_coldkey_balance(&contributions[1].0);
        let beneficiary_balance_before = SubtensorModule::get_coldkey_balance(&beneficiary);
        let accumulated_dividends_before = AccumulatedLeaseDividends::<Test>::get(lease_id);

        // Try to distribute the dividends
        let owner_cut_alpha = AlphaCurrency::from(5_000_000);
        SubtensorModule::distribute_leased_network_dividends(lease_id, owner_cut_alpha);

        // Ensure the dividends were not distributed
        assert_eq!(SubnetTAO::<Test>::get(lease.netuid), subnet_tao_before);
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[0].0),
            contributor1_balance_before
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[1].0),
            contributor2_balance_before
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&beneficiary),
            beneficiary_balance_before
        );
        // Ensure nothing was accumulated for later distribution
        assert_eq!(
            AccumulatedLeaseDividends::<Test>::get(lease_id),
            accumulated_dividends_before
        );
    });
}

#[test]
fn test_distribute_lease_network_dividends_accumulates_if_amount_is_too_low() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];

        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let emissions_share = Percent::from_percent(30);
        let (lease_id, lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            None, // We don't add any liquidity
        );

        let subnet_tao_before = SubnetTAO::<Test>::get(lease.netuid);
        let contributor1_balance_before = SubtensorModule::get_coldkey_balance(&contributions[0].0);
        let contributor2_balance_before = SubtensorModule::get_coldkey_balance(&contributions[1].0);
        let beneficiary_balance_before = SubtensorModule::get_coldkey_balance(&beneficiary);

        // Try to distribute the dividends
        let owner_cut_alpha = AlphaCurrency::from(5_000);
        SubtensorModule::distribute_leased_network_dividends(lease_id, owner_cut_alpha);

        // Ensure the dividends were not distributed
        assert_eq!(SubnetTAO::<Test>::get(lease.netuid), subnet_tao_before);
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[0].0),
            contributor1_balance_before
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[1].0),
            contributor2_balance_before
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&beneficiary),
            beneficiary_balance_before
        );
        // Ensure the correct amount of alpha was accumulated for later dividends distribution
        assert_eq!(
            AccumulatedLeaseDividends::<Test>::get(lease_id),
            emissions_share.mul_ceil(owner_cut_alpha.to_u64()).into()
        );
    });
}

#[test]
fn test_distribute_lease_network_dividends_accumulates_if_insufficient_liquidity() {
    new_test_ext(1).execute_with(|| {
        // Setup a crowdloan
        let crowdloan_id = 0;
        let beneficiary = U256::from(1);
        let deposit = 10_000_000_000; // 10 TAO
        let cap = 1_000_000_000_000; // 1000 TAO
        let contributions = vec![
            (U256::from(2), 600_000_000_000), // 600 TAO
            (U256::from(3), 390_000_000_000), // 390 TAO
        ];

        setup_crowdloan(crowdloan_id, deposit, cap, beneficiary, &contributions);

        // Setup a leased network
        let end_block = 500;
        let emissions_share = Percent::from_percent(30);
        let (lease_id, lease) = setup_leased_network(
            beneficiary,
            emissions_share,
            Some(end_block),
            None, // We don't add any liquidity
        );

        let subnet_tao_before = SubnetTAO::<Test>::get(lease.netuid);
        let contributor1_balance_before = SubtensorModule::get_coldkey_balance(&contributions[0].0);
        let contributor2_balance_before = SubtensorModule::get_coldkey_balance(&contributions[1].0);
        let beneficiary_balance_before = SubtensorModule::get_coldkey_balance(&beneficiary);

        // Try to distribute the dividends
        let owner_cut_alpha = AlphaCurrency::from(5_000_000);
        SubtensorModule::distribute_leased_network_dividends(lease_id, owner_cut_alpha);

        // Ensure the dividends were not distributed
        assert_eq!(SubnetTAO::<Test>::get(lease.netuid), subnet_tao_before);
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[0].0),
            contributor1_balance_before
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&contributions[1].0),
            contributor2_balance_before
        );
        assert_eq!(
            SubtensorModule::get_coldkey_balance(&beneficiary),
            beneficiary_balance_before
        );
        // Ensure the correct amount of alpha was accumulated for later dividends distribution
        assert_eq!(
            AccumulatedLeaseDividends::<Test>::get(lease_id),
            emissions_share.mul_ceil(owner_cut_alpha.to_u64()).into()
        );
    });
}

fn setup_crowdloan(
    id: u32,
    deposit: u64,
    cap: u64,
    beneficiary: U256,
    contributions: &[(U256, u64)],
) {
    let funds_account = U256::from(42424242 + id);

    pallet_crowdloan::Crowdloans::<Test>::insert(
        id,
        pallet_crowdloan::CrowdloanInfo {
            creator: beneficiary,
            deposit,
            min_contribution: 0,
            end: 0,
            cap,
            raised: cap,
            finalized: false,
            funds_account,
            call: None,
            target_address: None,
            contributors_count: 1 + contributions.len() as u32,
        },
    );

    // Simulate contributions
    pallet_crowdloan::Contributions::<Test>::insert(id, beneficiary, deposit);
    for (contributor, amount) in contributions {
        pallet_crowdloan::Contributions::<Test>::insert(id, contributor, amount);
    }

    SubtensorModule::add_balance_to_coldkey_account(&funds_account, cap);

    // Mark the crowdloan as finalizing
    pallet_crowdloan::CurrentCrowdloanId::<Test>::set(Some(0));
}

fn setup_leased_network(
    beneficiary: U256,
    emissions_share: Percent,
    end_block: Option<u64>,
    tao_to_stake: Option<u64>,
) -> (u32, SubnetLeaseOf<Test>) {
    let lease_id = 0;
    assert_ok!(SubtensorModule::do_register_leased_network(
        RuntimeOrigin::signed(beneficiary),
        emissions_share,
        end_block,
    ));

    // Configure subnet and add some stake
    let lease = SubnetLeases::<Test>::get(lease_id).unwrap();
    let netuid = lease.netuid;
    SubtokenEnabled::<Test>::insert(netuid, true);

    if let Some(tao_to_stake) = tao_to_stake {
        SubtensorModule::add_balance_to_coldkey_account(&lease.coldkey, tao_to_stake);
        assert_ok!(SubtensorModule::add_stake(
            RuntimeOrigin::signed(lease.coldkey),
            lease.hotkey,
            netuid,
            tao_to_stake
        ));
    }

    (lease_id, lease)
}
