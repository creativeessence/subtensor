#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::arithmetic_side_effects)]

use approx::assert_abs_diff_eq;
use frame_support::{assert_err, assert_noop, assert_ok};
use sp_arithmetic::helpers_128bit;
use sp_runtime::DispatchError;
use substrate_fixed::types::U96F32;
use subtensor_runtime_common::NetUid;

use super::*;
use crate::{OrderType, SqrtPrice, mock::*};

// this function is used to convert price (NON-SQRT price!) to TickIndex. it's only utility for
// testing, all the implementation logic is based on sqrt prices
fn price_to_tick(price: f64) -> TickIndex {
    let price_sqrt: SqrtPrice = SqrtPrice::from_num(price.sqrt());
    // Handle potential errors in the conversion
    match TickIndex::try_from_sqrt_price(price_sqrt) {
        Ok(mut tick) => {
            // Ensure the tick is within bounds
            if tick > TickIndex::MAX {
                tick = TickIndex::MAX;
            } else if tick < TickIndex::MIN {
                tick = TickIndex::MIN;
            }
            tick
        }
        // Default to a reasonable value when conversion fails
        Err(_) => {
            if price > 1.0 {
                TickIndex::MAX
            } else {
                TickIndex::MIN
            }
        }
    }
}

fn get_ticked_prices_around_current_price() -> (f64, f64) {
    // Get current price, ticks around it, and prices on the tick edges for test cases
    let netuid = NetUid::from(1);
    assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));
    let current_tick = CurrentTick::<Test>::get(netuid);

    // Low and high prices that match to a lower and higher tick that doesn't contain the current price
    let current_price_low_sqrt = current_tick.as_sqrt_price_bounded();
    let current_price_high_sqrt = current_tick.next().unwrap().as_sqrt_price_bounded();
    let current_price_low = U96F32::from_num(current_price_low_sqrt * current_price_low_sqrt);
    let current_price_high = U96F32::from_num(current_price_high_sqrt * current_price_high_sqrt);

    (
        current_price_low.to_num::<f64>(),
        current_price_high.to_num::<f64>() + 0.000000001,
    )
}

// this function is used to convert tick index NON-SQRT (!) price. it's only utility for
// testing, all the implementation logic is based on sqrt prices
fn tick_to_price(tick: TickIndex) -> f64 {
    // Handle errors gracefully
    match tick.try_to_sqrt_price() {
        Ok(price_sqrt) => (price_sqrt * price_sqrt).to_num::<f64>(),
        Err(_) => {
            // Return a sensible default based on whether the tick is above or below the valid range
            if tick > TickIndex::MAX {
                tick_to_price(TickIndex::MAX) // Use the max valid tick price
            } else {
                tick_to_price(TickIndex::MIN) // Use the min valid tick price
            }
        }
    }
}

mod dispatchables {
    use super::*;

    #[test]
    fn test_set_fee_rate() {
        new_test_ext().execute_with(|| {
            let netuid = NetUid::from(1);
            let fee_rate = 500; // 0.76% fee

            assert_noop!(
                Swap::set_fee_rate(RuntimeOrigin::signed(666), netuid, fee_rate),
                DispatchError::BadOrigin
            );

            assert_ok!(Swap::set_fee_rate(RuntimeOrigin::root(), netuid, fee_rate));

            // Check that fee rate was set correctly
            assert_eq!(FeeRate::<Test>::get(netuid), fee_rate);

            let fee_rate = fee_rate * 2;
            assert_ok!(Swap::set_fee_rate(
                RuntimeOrigin::signed(1),
                netuid,
                fee_rate
            ));
            assert_eq!(FeeRate::<Test>::get(netuid), fee_rate);

            // Verify fee rate validation - should fail if too high
            let too_high_fee = MaxFeeRate::get() + 1;
            assert_noop!(
                Swap::set_fee_rate(RuntimeOrigin::root(), netuid, too_high_fee),
                Error::<Test>::FeeRateTooHigh
            );
        });
    }

    #[test]
    fn test_toggle_user_liquidity() {
        new_test_ext().execute_with(|| {
            let netuid = NetUid::from(101);

            assert!(!EnabledUserLiquidity::<Test>::get(netuid));

            assert_ok!(Swap::toggle_user_liquidity(
                RuntimeOrigin::root(),
                netuid.into(),
                true
            ));

            assert!(EnabledUserLiquidity::<Test>::get(netuid));

            assert_noop!(
                Swap::toggle_user_liquidity(RuntimeOrigin::signed(666), netuid.into(), true),
                DispatchError::BadOrigin
            );

            assert_ok!(Swap::toggle_user_liquidity(
                RuntimeOrigin::signed(1),
                netuid.into(),
                true
            ));

            assert_noop!(
                Swap::toggle_user_liquidity(
                    RuntimeOrigin::root(),
                    NON_EXISTENT_NETUID.into(),
                    true
                ),
                Error::<Test>::SubNetworkDoesNotExist
            );
        });
    }
}

#[test]
fn test_swap_initialization() {
    new_test_ext().execute_with(|| {
        let netuid = NetUid::from(1);

        // Get reserves from the mock provider
        let tao = MockLiquidityProvider::tao_reserve(netuid.into());
        let alpha = MockLiquidityProvider::alpha_reserve(netuid.into());

        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        assert!(SwapV3Initialized::<Test>::get(netuid));

        // Verify current price is set
        let sqrt_price = AlphaSqrtPrice::<Test>::get(netuid);
        let expected_sqrt_price = U64F64::from_num(0.5_f64);
        assert_abs_diff_eq!(
            sqrt_price.to_num::<f64>(),
            expected_sqrt_price.to_num::<f64>(),
            epsilon = 0.000000001
        );

        // Verify that current tick is set
        let current_tick = CurrentTick::<Test>::get(netuid);
        let expected_current_tick = TickIndex::from_sqrt_price_bounded(expected_sqrt_price);
        assert_eq!(current_tick, expected_current_tick);

        // Calculate expected liquidity
        let expected_liquidity =
            helpers_128bit::sqrt((tao as u128).saturating_mul(u64::from(alpha) as u128)) as u64;

        // Get the protocol account
        let protocol_account_id = Pallet::<Test>::protocol_account_id();

        // Verify position created for protocol account
        let positions = Positions::<Test>::iter_prefix_values((netuid, protocol_account_id))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);

        let position = &positions[0];
        assert_eq!(position.liquidity, expected_liquidity);
        assert_eq!(position.tick_low, TickIndex::MIN);
        assert_eq!(position.tick_high, TickIndex::MAX);
        assert_eq!(position.fees_tao, 0);
        assert_eq!(position.fees_alpha, 0);

        // Verify ticks were created
        let tick_low = Ticks::<Test>::get(netuid, TickIndex::MIN).unwrap();
        let tick_high = Ticks::<Test>::get(netuid, TickIndex::MAX).unwrap();

        // Check liquidity values
        assert_eq!(tick_low.liquidity_net, expected_liquidity as i128);
        assert_eq!(tick_low.liquidity_gross, expected_liquidity);
        assert_eq!(tick_high.liquidity_net, -(expected_liquidity as i128));
        assert_eq!(tick_high.liquidity_gross, expected_liquidity);

        // Verify current liquidity is set
        assert_eq!(CurrentLiquidity::<Test>::get(netuid), expected_liquidity);
    });
}

// Test adding liquidity on top of the existing protocol liquidity
#[test]
fn test_add_liquidity_basic() {
    new_test_ext().execute_with(|| {
        let min_price = tick_to_price(TickIndex::MIN);
        let max_price = tick_to_price(TickIndex::MAX);
        let max_tick = price_to_tick(max_price);
        assert_eq!(max_tick, TickIndex::MAX);

        assert_ok!(Pallet::<Test>::maybe_initialize_v3(NetUid::from(1)));
        let current_price = Pallet::<Test>::current_price(NetUid::from(1)).to_num::<f64>();
        let (current_price_low, current_price_high) = get_ticked_prices_around_current_price();

        // As a user add liquidity with all possible corner cases
        //   - Initial price is 0.25
        //   - liquidity is expressed in RAO units
        // Test case is (price_low, price_high, liquidity, tao, alpha)
        [
            // Repeat the protocol liquidity at maximum range: Expect all the same values
            (
                min_price,
                max_price,
                2_000_000_000_u64,
                1_000_000_000_u64,
                4_000_000_000_u64,
            ),
            // Repeat the protocol liquidity at current to max range: Expect the same alpha
            (
                current_price_high,
                max_price,
                2_000_000_000_u64,
                0,
                4_000_000_000,
            ),
            // Repeat the protocol liquidity at min to current range: Expect all the same tao
            (
                min_price,
                current_price_low,
                2_000_000_000_u64,
                1_000_000_000,
                0,
            ),
            // Half to double price - just some sane wothdraw amounts
            (0.125, 0.5, 2_000_000_000_u64, 293_000_000, 1_171_000_000),
            // Both below price - tao is non-zero, alpha is zero
            (0.12, 0.13, 2_000_000_000_u64, 28_270_000, 0),
            // Both above price - tao is zero, alpha is non-zero
            (0.3, 0.4, 2_000_000_000_u64, 0, 489_200_000),
        ]
        .into_iter()
        .enumerate()
        .map(|(n, v)| (NetUid::from(n as u16 + 1), v.0, v.1, v.2, v.3, v.4))
        .for_each(
            |(netuid, price_low, price_high, liquidity, expected_tao, expected_alpha)| {
                assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

                // Calculate ticks (assuming tick math is tested separately)
                let tick_low = price_to_tick(price_low);
                let tick_high = price_to_tick(price_high);

                // Get tick infos and liquidity before adding (to account for protocol liquidity)
                let tick_low_info_before = Ticks::<Test>::get(netuid, tick_low).unwrap_or_default();
                let tick_high_info_before =
                    Ticks::<Test>::get(netuid, tick_high).unwrap_or_default();
                let liquidity_before = CurrentLiquidity::<Test>::get(netuid);

                // Add liquidity
                let (position_id, tao, alpha) = Pallet::<Test>::do_add_liquidity(
                    netuid,
                    &OK_COLDKEY_ACCOUNT_ID,
                    &OK_HOTKEY_ACCOUNT_ID,
                    tick_low,
                    tick_high,
                    liquidity,
                )
                .unwrap();

                assert_abs_diff_eq!(tao, expected_tao, epsilon = tao / 1000);
                assert_abs_diff_eq!(alpha, expected_alpha, epsilon = alpha / 1000);

                // Check that low and high ticks appear in the state and are properly updated
                let tick_low_info = Ticks::<Test>::get(netuid, tick_low).unwrap();
                let tick_high_info = Ticks::<Test>::get(netuid, tick_high).unwrap();
                let expected_liquidity_net_low = liquidity as i128;
                let expected_liquidity_gross_low = liquidity;
                let expected_liquidity_net_high = -(liquidity as i128);
                let expected_liquidity_gross_high = liquidity;

                assert_eq!(
                    tick_low_info.liquidity_net - tick_low_info_before.liquidity_net,
                    expected_liquidity_net_low,
                );
                assert_eq!(
                    tick_low_info.liquidity_gross - tick_low_info_before.liquidity_gross,
                    expected_liquidity_gross_low,
                );
                assert_eq!(
                    tick_high_info.liquidity_net - tick_high_info_before.liquidity_net,
                    expected_liquidity_net_high,
                );
                assert_eq!(
                    tick_high_info.liquidity_gross - tick_high_info_before.liquidity_gross,
                    expected_liquidity_gross_high,
                );

                // Liquidity position at correct ticks
                assert_eq!(
                    Pallet::<Test>::count_positions(netuid, &OK_COLDKEY_ACCOUNT_ID),
                    1
                );

                let position =
                    Positions::<Test>::get((netuid, OK_COLDKEY_ACCOUNT_ID, position_id)).unwrap();
                assert_eq!(position.liquidity, liquidity);
                assert_eq!(position.tick_low, tick_low);
                assert_eq!(position.tick_high, tick_high);
                assert_eq!(position.fees_alpha, 0);
                assert_eq!(position.fees_tao, 0);

                // Current liquidity is updated only when price range includes the current price
                let expected_liquidity =
                    if (price_high > current_price) && (price_low <= current_price) {
                        liquidity_before + liquidity
                    } else {
                        liquidity_before
                    };

                assert_eq!(CurrentLiquidity::<Test>::get(netuid), expected_liquidity)
            },
        );
    });
}

#[test]
fn test_add_liquidity_out_of_bounds() {
    new_test_ext().execute_with(|| {
        [
            // For our tests, we'll construct TickIndex values that are intentionally
            // outside the valid range for testing purposes only
            (
                TickIndex::new_unchecked(TickIndex::MIN.get() - 1),
                TickIndex::MAX,
                1_000_000_000_u64,
            ),
            (
                TickIndex::MIN,
                TickIndex::new_unchecked(TickIndex::MAX.get() + 1),
                1_000_000_000_u64,
            ),
            (
                TickIndex::new_unchecked(TickIndex::MIN.get() - 1),
                TickIndex::new_unchecked(TickIndex::MAX.get() + 1),
                1_000_000_000_u64,
            ),
            (
                TickIndex::new_unchecked(TickIndex::MIN.get() - 100),
                TickIndex::new_unchecked(TickIndex::MAX.get() + 100),
                1_000_000_000_u64,
            ),
            // Inverted ticks: high < low
            (
                TickIndex::new_unchecked(-900),
                TickIndex::new_unchecked(-1000),
                1_000_000_000_u64,
            ),
            // Equal ticks: high == low
            (
                TickIndex::new_unchecked(-10_000),
                TickIndex::new_unchecked(-10_000),
                1_000_000_000_u64,
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(n, v)| (NetUid::from(n as u16 + 1), v.0, v.1, v.2))
        .for_each(|(netuid, tick_low, tick_high, liquidity)| {
            assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

            // Add liquidity
            assert_err!(
                Swap::do_add_liquidity(
                    netuid,
                    &OK_COLDKEY_ACCOUNT_ID,
                    &OK_HOTKEY_ACCOUNT_ID,
                    tick_low,
                    tick_high,
                    liquidity
                ),
                Error::<Test>::InvalidTickRange,
            );
        });
    });
}

#[test]
fn test_add_liquidity_over_balance() {
    new_test_ext().execute_with(|| {
        let coldkey_account_id = 3;
        let hotkey_account_id = 1002;

        [
            // Lower than price (not enough tao)
            (0.1, 0.2, 100_000_000_000_u64),
            // Higher than price (not enough alpha)
            (0.3, 0.4, 100_000_000_000_u64),
            // Around the price (not enough both)
            (0.1, 0.4, 100_000_000_000_u64),
        ]
        .into_iter()
        .enumerate()
        .map(|(n, v)| (NetUid::from(n as u16 + 1), v.0, v.1, v.2))
        .for_each(|(netuid, price_low, price_high, liquidity)| {
            // Calculate ticks
            let tick_low = price_to_tick(price_low);
            let tick_high = price_to_tick(price_high);

            assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

            // Add liquidity
            assert_err!(
                Pallet::<Test>::do_add_liquidity(
                    netuid,
                    &coldkey_account_id,
                    &hotkey_account_id,
                    tick_low,
                    tick_high,
                    liquidity
                ),
                Error::<Test>::InsufficientBalance,
            );
        });
    });
}

// cargo test --package pallet-subtensor-swap --lib -- pallet::impls::tests::test_remove_liquidity_basic --exact --show-output
#[test]
fn test_remove_liquidity_basic() {
    new_test_ext().execute_with(|| {
        let min_price = tick_to_price(TickIndex::MIN);
        let max_price = tick_to_price(TickIndex::MAX);
        let max_tick = price_to_tick(max_price);
        assert_eq!(max_tick, TickIndex::MAX);

        let (current_price_low, current_price_high) = get_ticked_prices_around_current_price();

        // As a user add liquidity with all possible corner cases
        //   - Initial price is 0.25
        //   - liquidity is expressed in RAO units
        // Test case is (price_low, price_high, liquidity, tao, alpha)
        [
            // Repeat the protocol liquidity at maximum range: Expect all the same values
            (
                min_price,
                max_price,
                2_000_000_000_u64,
                1_000_000_000_u64,
                4_000_000_000_u64,
            ),
            // Repeat the protocol liquidity at current to max range: Expect the same alpha
            (
                current_price_high,
                max_price,
                2_000_000_000_u64,
                0,
                4_000_000_000,
            ),
            // Repeat the protocol liquidity at min to current range: Expect all the same tao
            (
                min_price,
                current_price_low,
                2_000_000_000_u64,
                1_000_000_000,
                0,
            ),
            // Half to double price - just some sane wothdraw amounts
            (0.125, 0.5, 2_000_000_000_u64, 293_000_000, 1_171_000_000),
            // Both below price - tao is non-zero, alpha is zero
            (0.12, 0.13, 2_000_000_000_u64, 28_270_000, 0),
            // Both above price - tao is zero, alpha is non-zero
            (0.3, 0.4, 2_000_000_000_u64, 0, 489_200_000),
        ]
        .into_iter()
        .enumerate()
        .map(|(n, v)| (NetUid::from(n as u16 + 1), v.0, v.1, v.2, v.3, v.4))
        .for_each(|(netuid, price_low, price_high, liquidity, tao, alpha)| {
            // Calculate ticks (assuming tick math is tested separately)
            let tick_low = price_to_tick(price_low);
            let tick_high = price_to_tick(price_high);

            assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));
            let liquidity_before = CurrentLiquidity::<Test>::get(netuid);

            // Add liquidity
            let (position_id, _, _) = Pallet::<Test>::do_add_liquidity(
                netuid,
                &OK_COLDKEY_ACCOUNT_ID,
                &OK_HOTKEY_ACCOUNT_ID,
                tick_low,
                tick_high,
                liquidity,
            )
            .unwrap();

            // Remove liquidity
            let remove_result =
                Pallet::<Test>::do_remove_liquidity(netuid, &OK_COLDKEY_ACCOUNT_ID, position_id)
                    .unwrap();
            assert_abs_diff_eq!(remove_result.tao, tao, epsilon = tao / 1000);
            assert_abs_diff_eq!(
                u64::from(remove_result.alpha),
                alpha,
                epsilon = alpha / 1000
            );
            assert_eq!(remove_result.fee_tao, 0);
            assert_eq!(remove_result.fee_alpha, 0.into());

            // Liquidity position is removed
            assert_eq!(
                Pallet::<Test>::count_positions(netuid, &OK_COLDKEY_ACCOUNT_ID),
                0
            );
            assert!(Positions::<Test>::get((netuid, OK_COLDKEY_ACCOUNT_ID, position_id)).is_none());

            // Current liquidity is updated (back where it was)
            assert_eq!(CurrentLiquidity::<Test>::get(netuid), liquidity_before);
        });
    });
}

#[test]
fn test_remove_liquidity_nonexisting_position() {
    new_test_ext().execute_with(|| {
        let min_price = tick_to_price(TickIndex::MIN);
        let max_price = tick_to_price(TickIndex::MAX);
        let max_tick = price_to_tick(max_price);
        assert_eq!(max_tick.get(), TickIndex::MAX.get());

        let liquidity = 2_000_000_000_u64;
        let netuid = NetUid::from(1);

        // Calculate ticks (assuming tick math is tested separately)
        let tick_low = price_to_tick(min_price);
        let tick_high = price_to_tick(max_price);

        // Setup swap
        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        // Add liquidity
        assert_ok!(Pallet::<Test>::do_add_liquidity(
            netuid,
            &OK_COLDKEY_ACCOUNT_ID,
            &OK_HOTKEY_ACCOUNT_ID,
            tick_low,
            tick_high,
            liquidity,
        ));

        assert!(Pallet::<Test>::count_positions(netuid, &OK_COLDKEY_ACCOUNT_ID) > 0);

        // Remove liquidity
        assert_err!(
            Pallet::<Test>::do_remove_liquidity(
                netuid,
                &OK_COLDKEY_ACCOUNT_ID,
                PositionId::new::<Test>()
            ),
            Error::<Test>::LiquidityNotFound,
        );
    });
}

// cargo test --package pallet-subtensor-swap --lib -- pallet::tests::test_modify_position_basic --exact --show-output
#[test]
fn test_modify_position_basic() {
    new_test_ext().execute_with(|| {
        let max_price = tick_to_price(TickIndex::MAX);
        let max_tick = price_to_tick(max_price);
        let limit_price = 1000.0_f64;
        assert_eq!(max_tick, TickIndex::MAX);
        let (current_price_low, _current_price_high) = get_ticked_prices_around_current_price();

        // As a user add liquidity with all possible corner cases
        //   - Initial price is 0.25
        //   - liquidity is expressed in RAO units
        // Test case is (price_low, price_high, liquidity, tao, alpha)
        [
            // Repeat the protocol liquidity at current to max range: Expect the same alpha
            (
                current_price_low,
                max_price,
                2_000_000_000_u64,
                4_000_000_000,
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(n, v)| (NetUid::from(n as u16 + 1), v.0, v.1, v.2, v.3))
        .for_each(|(netuid, price_low, price_high, liquidity, alpha)| {
            // Calculate ticks (assuming tick math is tested separately)
            let tick_low = price_to_tick(price_low);
            let tick_high = price_to_tick(price_high);

            assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

            // Add liquidity
            let (position_id, _, _) = Pallet::<Test>::do_add_liquidity(
                netuid,
                &OK_COLDKEY_ACCOUNT_ID,
                &OK_HOTKEY_ACCOUNT_ID,
                tick_low,
                tick_high,
                liquidity,
            )
            .unwrap();

            // Get tick infos before the swap/update
            let tick_low_info_before = Ticks::<Test>::get(netuid, tick_low).unwrap();
            let tick_high_info_before = Ticks::<Test>::get(netuid, tick_high).unwrap();

            // Swap to create fees on the position
            let sqrt_limit_price = SqrtPrice::from_num((limit_price).sqrt());
            Pallet::<Test>::do_swap(
                netuid,
                OrderType::Buy,
                liquidity / 10,
                sqrt_limit_price,
                false,
                false,
            )
            .unwrap();

            // Modify liquidity (also causes claiming of fees)
            let liquidity_before = CurrentLiquidity::<Test>::get(netuid);
            let modify_result = Pallet::<Test>::do_modify_position(
                netuid,
                &OK_COLDKEY_ACCOUNT_ID,
                &OK_HOTKEY_ACCOUNT_ID,
                position_id,
                -((liquidity / 10) as i64),
            )
            .unwrap();
            assert_abs_diff_eq!(
                u64::from(modify_result.alpha),
                alpha / 10,
                epsilon = alpha / 1000
            );
            assert!(modify_result.fee_tao > 0);
            assert_eq!(modify_result.fee_alpha, 0.into());

            // Liquidity position is reduced
            assert_eq!(
                Pallet::<Test>::count_positions(netuid, &OK_COLDKEY_ACCOUNT_ID),
                1
            );

            // Current liquidity is reduced with modify_position
            assert!(CurrentLiquidity::<Test>::get(netuid) < liquidity_before);

            // Position liquidity is reduced
            let position =
                Positions::<Test>::get((netuid, OK_COLDKEY_ACCOUNT_ID, position_id)).unwrap();
            assert_eq!(position.liquidity, liquidity * 9 / 10);
            assert_eq!(position.tick_low, tick_low);
            assert_eq!(position.tick_high, tick_high);

            // Tick liquidity is updated properly for low and high position ticks
            let tick_low_info_after = Ticks::<Test>::get(netuid, tick_low).unwrap();
            let tick_high_info_after = Ticks::<Test>::get(netuid, tick_high).unwrap();

            assert_eq!(
                tick_low_info_before.liquidity_net - (liquidity / 10) as i128,
                tick_low_info_after.liquidity_net,
            );
            assert_eq!(
                tick_low_info_before.liquidity_gross - (liquidity / 10),
                tick_low_info_after.liquidity_gross,
            );
            assert_eq!(
                tick_high_info_before.liquidity_net + (liquidity / 10) as i128,
                tick_high_info_after.liquidity_net,
            );
            assert_eq!(
                tick_high_info_before.liquidity_gross - (liquidity / 10),
                tick_high_info_after.liquidity_gross,
            );

            // Modify liquidity again (ensure fees aren't double-collected)
            let modify_result = Pallet::<Test>::do_modify_position(
                netuid,
                &OK_COLDKEY_ACCOUNT_ID,
                &OK_HOTKEY_ACCOUNT_ID,
                position_id,
                -((liquidity / 100) as i64),
            )
            .unwrap();

            assert_abs_diff_eq!(
                u64::from(modify_result.alpha),
                alpha / 100,
                epsilon = alpha / 1000
            );
            assert_eq!(modify_result.fee_tao, 0);
            assert_eq!(modify_result.fee_alpha, 0.into());
        });
    });
}

// cargo test --package pallet-subtensor-swap --lib -- pallet::impls::tests::test_swap_basic --exact --show-output
#[test]
fn test_swap_basic() {
    new_test_ext().execute_with(|| {
        // Current price is 0.25
        // Test case is (order_type, liquidity, limit_price, output_amount)
        [
            (OrderType::Buy, 1_000u64, 1000.0_f64, 3990_u64),
            (OrderType::Sell, 1_000u64, 0.0001_f64, 250_u64),
            (OrderType::Buy, 500_000_000, 1000.0, 2_000_000_000),
        ]
        .into_iter()
        .enumerate()
        .map(|(n, v)| (NetUid::from(n as u16 + 1), v.0, v.1, v.2, v.3))
        .for_each(
            |(netuid, order_type, liquidity, limit_price, output_amount)| {
                // Consumed liquidity ticks
                let tick_low = TickIndex::MIN;
                let tick_high = TickIndex::MAX;

                // Setup swap
                assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

                // Get tick infos before the swap
                let tick_low_info_before = Ticks::<Test>::get(netuid, tick_low).unwrap_or_default();
                let tick_high_info_before =
                    Ticks::<Test>::get(netuid, tick_high).unwrap_or_default();
                let liquidity_before = CurrentLiquidity::<Test>::get(netuid);

                // Get current price
                let current_price = Pallet::<Test>::current_price(netuid);

                // Swap
                let sqrt_limit_price = SqrtPrice::from_num((limit_price).sqrt());
                let swap_result = Pallet::<Test>::do_swap(
                    netuid,
                    order_type,
                    liquidity,
                    sqrt_limit_price,
                    false,
                    false,
                )
                .unwrap();
                assert_abs_diff_eq!(
                    swap_result.amount_paid_out,
                    output_amount,
                    epsilon = output_amount / 100
                );

                let (tao_delta_expected, alpha_delta_expected) = match order_type {
                    OrderType::Buy => (liquidity as i64, -(output_amount as i64)),
                    OrderType::Sell => (-(output_amount as i64), liquidity as i64),
                };

                assert_abs_diff_eq!(
                    swap_result.alpha_reserve_delta,
                    alpha_delta_expected,
                    epsilon = alpha_delta_expected.abs() / 10
                );
                assert_abs_diff_eq!(
                    swap_result.tao_reserve_delta,
                    tao_delta_expected,
                    epsilon = tao_delta_expected.abs() / 10
                );

                // Check that low and high ticks' fees were updated properly, and liquidity values were not updated
                let tick_low_info = Ticks::<Test>::get(netuid, tick_low).unwrap();
                let tick_high_info = Ticks::<Test>::get(netuid, tick_high).unwrap();
                let expected_liquidity_net_low = tick_low_info_before.liquidity_net;
                let expected_liquidity_gross_low = tick_low_info_before.liquidity_gross;
                let expected_liquidity_net_high = tick_high_info_before.liquidity_net;
                let expected_liquidity_gross_high = tick_high_info_before.liquidity_gross;
                assert_eq!(tick_low_info.liquidity_net, expected_liquidity_net_low,);
                assert_eq!(tick_low_info.liquidity_gross, expected_liquidity_gross_low,);
                assert_eq!(tick_high_info.liquidity_net, expected_liquidity_net_high,);
                assert_eq!(
                    tick_high_info.liquidity_gross,
                    expected_liquidity_gross_high,
                );

                // Expected fee amount
                let fee_rate = FeeRate::<Test>::get(netuid) as f64 / u16::MAX as f64;
                let expected_fee = (liquidity as f64 * fee_rate) as u64;

                // Global fees should be updated
                let actual_global_fee = ((match order_type {
                    OrderType::Buy => FeeGlobalTao::<Test>::get(netuid),
                    OrderType::Sell => FeeGlobalAlpha::<Test>::get(netuid),
                })
                .to_num::<f64>()
                    * (liquidity_before as f64)) as u64;

                assert!((swap_result.fee_paid as i64 - expected_fee as i64).abs() <= 1);
                assert!((actual_global_fee as i64 - expected_fee as i64).abs() <= 1);

                // Tick fees should be updated

                // Liquidity position should not be updated
                let protocol_id = Pallet::<Test>::protocol_account_id();
                let positions = Positions::<Test>::iter_prefix_values((netuid, protocol_id))
                    .collect::<Vec<_>>();
                let position = positions.first().unwrap();

                assert_eq!(
                    position.liquidity,
                    helpers_128bit::sqrt(
                        MockLiquidityProvider::tao_reserve(netuid.into()) as u128
                            * u64::from(MockLiquidityProvider::alpha_reserve(netuid.into()))
                                as u128
                    ) as u64
                );
                assert_eq!(position.tick_low, tick_low);
                assert_eq!(position.tick_high, tick_high);
                assert_eq!(position.fees_alpha, 0);
                assert_eq!(position.fees_tao, 0);

                // Current liquidity is not updated
                assert_eq!(CurrentLiquidity::<Test>::get(netuid), liquidity_before);

                // Assert that price movement is in correct direction
                let sqrt_current_price_after = Pallet::<Test>::current_price_sqrt(netuid);
                let current_price_after = Pallet::<Test>::current_price(netuid);
                match order_type {
                    OrderType::Buy => assert!(current_price_after >= current_price),
                    OrderType::Sell => assert!(current_price_after <= current_price),
                }

                // Assert that current tick is updated
                let current_tick = CurrentTick::<Test>::get(netuid);
                let expected_current_tick =
                    TickIndex::from_sqrt_price_bounded(sqrt_current_price_after);
                assert_eq!(current_tick, expected_current_tick);
            },
        );
    });
}

// In this test the swap starts and ends within one (large liquidity) position
// SKIP_WASM_BUILD=1 RUST_LOG=debug cargo test --package pallet-subtensor-swap --lib -- pallet::tests::test_swap_single_position --exact --show-output
#[test]
fn test_swap_single_position() {
    let min_price = tick_to_price(TickIndex::MIN);
    let max_price = tick_to_price(TickIndex::MAX);
    let max_tick = price_to_tick(max_price);
    let netuid = NetUid::from(1);
    assert_eq!(max_tick, TickIndex::MAX);

    let mut current_price_low = 0_f64;
    let mut current_price_high = 0_f64;
    let mut current_price = 0_f64;
    new_test_ext().execute_with(|| {
        let (low, high) = get_ticked_prices_around_current_price();
        current_price_low = low;
        current_price_high = high;
        current_price = Pallet::<Test>::current_price(netuid).to_num::<f64>();
    });

    // Current price is 0.25
    // The test case is based on the current price and position prices are defined as a price
    // offset from the current price
    // Outer part of test case is Position: (price_low_offset, price_high_offset, liquidity)
    [
        // Very localized position at the current price
        (-0.1, 0.1, 500_000_000_000_u64),
        // Repeat the protocol liquidity at maximum range
        (
            min_price - current_price,
            max_price - current_price,
            2_000_000_000_u64,
        ),
        // Repeat the protocol liquidity at current to max range
        (
            current_price_high - current_price,
            max_price - current_price,
            2_000_000_000_u64,
        ),
        // Repeat the protocol liquidity at min to current range
        (
            min_price - current_price,
            current_price_low - current_price,
            2_000_000_000_u64,
        ),
        // Half to double price
        (-0.125, 0.25, 2_000_000_000_u64),
        // A few other price ranges and liquidity volumes
        (-0.1, 0.1, 2_000_000_000_u64),
        (-0.1, 0.1, 10_000_000_000_u64),
        (-0.1, 0.1, 100_000_000_000_u64),
        (-0.01, 0.01, 100_000_000_000_u64),
        (-0.001, 0.001, 100_000_000_000_u64),
    ]
    .into_iter()
    .for_each(
        |(price_low_offset, price_high_offset, position_liquidity)| {
            // Inner part of test case is Order: (order_type, order_liquidity, limit_price)
            // order_liquidity is represented as a fraction of position_liquidity
            [
                (OrderType::Buy, 0.0001, 1000.0_f64),
                (OrderType::Sell, 0.0001, 0.0001_f64),
                (OrderType::Buy, 0.001, 1000.0_f64),
                (OrderType::Sell, 0.001, 0.0001_f64),
                (OrderType::Buy, 0.01, 1000.0_f64),
                (OrderType::Sell, 0.01, 0.0001_f64),
                (OrderType::Buy, 0.1, 1000.0_f64),
                (OrderType::Sell, 0.1, 0.0001),
                (OrderType::Buy, 0.2, 1000.0_f64),
                (OrderType::Sell, 0.2, 0.0001),
                (OrderType::Buy, 0.5, 1000.0),
                (OrderType::Sell, 0.5, 0.0001),
            ]
            .into_iter()
            .for_each(|(order_type, order_liquidity_fraction, limit_price)| {
                new_test_ext().execute_with(|| {
                    //////////////////////////////////////////////
                    // Initialize pool and add the user position
                    assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));
                    let tao_reserve = MockLiquidityProvider::tao_reserve(netuid.into());
                    let alpha_reserve =
                        u64::from(MockLiquidityProvider::alpha_reserve(netuid.into()));
                    let protocol_liquidity = (tao_reserve as f64 * alpha_reserve as f64).sqrt();

                    // Add liquidity
                    let current_price = Pallet::<Test>::current_price(netuid).to_num::<f64>();
                    let sqrt_current_price =
                        Pallet::<Test>::current_price_sqrt(netuid).to_num::<f64>();

                    let price_low = price_low_offset + current_price;
                    let price_high = price_high_offset + current_price;
                    let tick_low = price_to_tick(price_low);
                    let tick_high = price_to_tick(price_high);
                    let (_position_id, _tao, _alpha) = Pallet::<Test>::do_add_liquidity(
                        netuid,
                        &OK_COLDKEY_ACCOUNT_ID,
                        &OK_HOTKEY_ACCOUNT_ID,
                        tick_low,
                        tick_high,
                        position_liquidity,
                    )
                    .unwrap();

                    // Liquidity position at correct ticks
                    assert_eq!(
                        Pallet::<Test>::count_positions(netuid, &OK_COLDKEY_ACCOUNT_ID),
                        1
                    );

                    // Get tick infos before the swap
                    let tick_low_info_before =
                        Ticks::<Test>::get(netuid, tick_low).unwrap_or_default();
                    let tick_high_info_before =
                        Ticks::<Test>::get(netuid, tick_high).unwrap_or_default();
                    let liquidity_before = CurrentLiquidity::<Test>::get(netuid);
                    assert_abs_diff_eq!(
                        liquidity_before as f64,
                        protocol_liquidity + position_liquidity as f64,
                        epsilon = liquidity_before as f64 / 1000.
                    );

                    //////////////////////////////////////////////
                    // Swap

                    // Calculate the expected output amount for the cornercase of one step
                    let order_liquidity = order_liquidity_fraction * position_liquidity as f64;

                    let output_amount = match order_type {
                        OrderType::Buy => {
                            let denom = sqrt_current_price
                                * (sqrt_current_price * liquidity_before as f64 + order_liquidity);
                            let per_order_liq = liquidity_before as f64 / denom;
                            per_order_liq * order_liquidity
                        }
                        OrderType::Sell => {
                            let denom =
                                liquidity_before as f64 / sqrt_current_price + order_liquidity;
                            let per_order_liq =
                                sqrt_current_price * liquidity_before as f64 / denom;
                            per_order_liq * order_liquidity
                        }
                    };

                    // Do the swap
                    let sqrt_limit_price = SqrtPrice::from_num((limit_price).sqrt());
                    let swap_result = Pallet::<Test>::do_swap(
                        netuid,
                        order_type,
                        order_liquidity as u64,
                        sqrt_limit_price,
                        false,
                        false,
                    )
                    .unwrap();
                    assert_abs_diff_eq!(
                        swap_result.amount_paid_out as f64,
                        output_amount,
                        epsilon = output_amount / 10.
                    );

                    if order_liquidity_fraction <= 0.001 {
                        let (tao_delta_expected, alpha_delta_expected) = match order_type {
                            OrderType::Buy => (order_liquidity as i64, -(output_amount as i64)),
                            OrderType::Sell => (-(output_amount as i64), order_liquidity as i64),
                        };
                        assert_abs_diff_eq!(
                            swap_result.alpha_reserve_delta,
                            alpha_delta_expected,
                            epsilon = alpha_delta_expected.abs() / 10
                        );
                        assert_abs_diff_eq!(
                            swap_result.tao_reserve_delta,
                            tao_delta_expected,
                            epsilon = tao_delta_expected.abs() / 10
                        );
                    }

                    // Assert that price movement is in correct direction
                    let current_price_after = Pallet::<Test>::current_price(netuid);
                    match order_type {
                        OrderType::Buy => assert!(current_price_after > current_price),
                        OrderType::Sell => assert!(current_price_after < current_price),
                    }

                    // Assert that for small amounts price stays within the user position
                    if (order_liquidity_fraction <= 0.001)
                        && (price_low_offset > 0.0001)
                        && (price_high_offset > 0.0001)
                    {
                        assert!(current_price_after <= price_high);
                        assert!(current_price_after >= price_low);
                    }

                    // Check that low and high ticks' fees were updated properly
                    let tick_low_info = Ticks::<Test>::get(netuid, tick_low).unwrap();
                    let tick_high_info = Ticks::<Test>::get(netuid, tick_high).unwrap();
                    let expected_liquidity_net_low = tick_low_info_before.liquidity_net;
                    let expected_liquidity_gross_low = tick_low_info_before.liquidity_gross;
                    let expected_liquidity_net_high = tick_high_info_before.liquidity_net;
                    let expected_liquidity_gross_high = tick_high_info_before.liquidity_gross;
                    assert_eq!(tick_low_info.liquidity_net, expected_liquidity_net_low,);
                    assert_eq!(tick_low_info.liquidity_gross, expected_liquidity_gross_low,);
                    assert_eq!(tick_high_info.liquidity_net, expected_liquidity_net_high,);
                    assert_eq!(
                        tick_high_info.liquidity_gross,
                        expected_liquidity_gross_high,
                    );

                    // Expected fee amount
                    let fee_rate = FeeRate::<Test>::get(netuid) as f64 / u16::MAX as f64;
                    let expected_fee =
                        (order_liquidity - order_liquidity / (1.0 + fee_rate)) as u64;

                    // Global fees should be updated
                    let actual_global_fee = ((match order_type {
                        OrderType::Buy => FeeGlobalTao::<Test>::get(netuid),
                        OrderType::Sell => FeeGlobalAlpha::<Test>::get(netuid),
                    })
                    .to_num::<f64>()
                        * (liquidity_before as f64))
                        as u64;

                    assert_abs_diff_eq!(
                        swap_result.fee_paid,
                        expected_fee,
                        epsilon = expected_fee / 10
                    );
                    assert_abs_diff_eq!(
                        actual_global_fee,
                        expected_fee,
                        epsilon = expected_fee / 10
                    );

                    // Tick fees should be updated

                    // Liquidity position should not be updated
                    let positions =
                        Positions::<Test>::iter_prefix_values((netuid, OK_COLDKEY_ACCOUNT_ID))
                            .collect::<Vec<_>>();
                    let position = positions.first().unwrap();

                    assert_eq!(position.liquidity, position_liquidity,);
                    assert_eq!(position.tick_low, tick_low);
                    assert_eq!(position.tick_high, tick_high);
                    assert_eq!(position.fees_alpha, 0);
                    assert_eq!(position.fees_tao, 0);
                });
            });
        },
    );
}

// This test is a sanity check for swap and multiple positions
// SKIP_WASM_BUILD=1 RUST_LOG=debug cargo test --package pallet-subtensor-swap --lib -- pallet::impls::tests::test_swap_multiple_positions --exact --show-output --nocapture
#[test]
fn test_swap_multiple_positions() {
    new_test_ext().execute_with(|| {
        let min_price = tick_to_price(TickIndex::MIN);
        let max_price = tick_to_price(TickIndex::MAX);
        let max_tick = price_to_tick(max_price);
        let netuid = NetUid::from(1);
        assert_eq!(max_tick, TickIndex::MAX);

        //////////////////////////////////////////////
        // Initialize pool and add the user position
        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        // Add liquidity
        let current_price = Pallet::<Test>::current_price(netuid).to_num::<f64>();

        // Current price is 0.25
        // All positions below are placed at once
        [
            // Very localized position at the current price
            (-0.1, 0.1, 500_000_000_000_u64),
            // Repeat the protocol liquidity at maximum range
            (
                min_price - current_price,
                max_price - current_price,
                2_000_000_000_u64,
            ),
            // Repeat the protocol liquidity at current to max range
            (0.0, max_price - current_price, 2_000_000_000_u64),
            // Repeat the protocol liquidity at min to current range
            (min_price - current_price, 0.0, 2_000_000_000_u64),
            // Half to double price
            (-0.125, 0.25, 2_000_000_000_u64),
            // A few other price ranges and liquidity volumes
            (-0.1, 0.1, 2_000_000_000_u64),
            (-0.1, 0.1, 10_000_000_000_u64),
            (-0.1, 0.1, 100_000_000_000_u64),
            (-0.01, 0.01, 100_000_000_000_u64),
            (-0.001, 0.001, 100_000_000_000_u64),
            // A few (overlapping) positions up the range
            (0.01, 0.02, 100_000_000_000_u64),
            (0.02, 0.03, 100_000_000_000_u64),
            (0.03, 0.04, 100_000_000_000_u64),
            (0.03, 0.05, 100_000_000_000_u64),
            // A few (overlapping) positions down the range
            (-0.02, -0.01, 100_000_000_000_u64),
            (-0.03, -0.02, 100_000_000_000_u64),
            (-0.04, -0.03, 100_000_000_000_u64),
            (-0.05, -0.03, 100_000_000_000_u64),
        ]
        .into_iter()
        .for_each(
            |(price_low_offset, price_high_offset, position_liquidity)| {
                let price_low = price_low_offset + current_price;
                let price_high = price_high_offset + current_price;
                let tick_low = price_to_tick(price_low);
                let tick_high = price_to_tick(price_high);
                let (_position_id, _tao, _alpha) = Pallet::<Test>::do_add_liquidity(
                    netuid,
                    &OK_COLDKEY_ACCOUNT_ID,
                    &OK_HOTKEY_ACCOUNT_ID,
                    tick_low,
                    tick_high,
                    position_liquidity,
                )
                .unwrap();
            },
        );

        // All these orders are executed without swap reset
        [
            (OrderType::Buy, 100_000_u64, 1000.0_f64),
            (OrderType::Sell, 100_000, 0.0001_f64),
            (OrderType::Buy, 1_000_000, 1000.0_f64),
            (OrderType::Sell, 1_000_000, 0.0001_f64),
            (OrderType::Buy, 10_000_000, 1000.0_f64),
            (OrderType::Sell, 10_000_000, 0.0001_f64),
            (OrderType::Buy, 100_000_000, 1000.0),
            (OrderType::Sell, 100_000_000, 0.0001),
            (OrderType::Buy, 200_000_000, 1000.0_f64),
            (OrderType::Sell, 200_000_000, 0.0001),
            (OrderType::Buy, 500_000_000, 1000.0),
            (OrderType::Sell, 500_000_000, 0.0001),
            (OrderType::Buy, 1_000_000_000, 1000.0),
            (OrderType::Sell, 1_000_000_000, 0.0001),
            (OrderType::Buy, 10_000_000_000, 1000.0),
            (OrderType::Sell, 10_000_000_000, 0.0001),
        ]
        .into_iter()
        .for_each(|(order_type, order_liquidity, limit_price)| {
            //////////////////////////////////////////////
            // Swap
            let sqrt_current_price = Pallet::<Test>::current_price_sqrt(netuid);
            let current_price = (sqrt_current_price * sqrt_current_price).to_num::<f64>();
            let liquidity_before = CurrentLiquidity::<Test>::get(netuid);

            let output_amount = match order_type {
                OrderType::Buy => {
                    let denom = sqrt_current_price.to_num::<f64>()
                        * (sqrt_current_price.to_num::<f64>() * liquidity_before as f64
                            + order_liquidity as f64);
                    let per_order_liq = liquidity_before as f64 / denom;
                    per_order_liq * order_liquidity as f64
                }
                OrderType::Sell => {
                    let denom = liquidity_before as f64 / sqrt_current_price.to_num::<f64>()
                        + order_liquidity as f64;
                    let per_order_liq =
                        sqrt_current_price.to_num::<f64>() * liquidity_before as f64 / denom;
                    per_order_liq * order_liquidity as f64
                }
            };

            // Do the swap
            let sqrt_limit_price = SqrtPrice::from_num((limit_price).sqrt());
            let swap_result = Pallet::<Test>::do_swap(
                netuid,
                order_type,
                order_liquidity,
                sqrt_limit_price,
                false,
                false,
            )
            .unwrap();
            assert_abs_diff_eq!(
                swap_result.amount_paid_out as f64,
                output_amount,
                epsilon = output_amount / 10.
            );

            let tao_reserve = MockLiquidityProvider::tao_reserve(netuid.into());
            let alpha_reserve = u64::from(MockLiquidityProvider::alpha_reserve(netuid.into()));
            let output_amount = output_amount as u64;

            assert!(output_amount > 0);

            if alpha_reserve > order_liquidity && tao_reserve > order_liquidity {
                let (tao_delta_expected, alpha_delta_expected) = match order_type {
                    OrderType::Buy => (order_liquidity as i64, -(output_amount as i64)),
                    OrderType::Sell => (-(output_amount as i64), order_liquidity as i64),
                };
                assert_abs_diff_eq!(
                    swap_result.alpha_reserve_delta,
                    alpha_delta_expected,
                    epsilon = alpha_delta_expected.abs() / 100
                );
                assert_abs_diff_eq!(
                    swap_result.tao_reserve_delta,
                    tao_delta_expected,
                    epsilon = tao_delta_expected.abs() / 100
                );
            }

            // Assert that price movement is in correct direction
            let sqrt_current_price_after = Pallet::<Test>::current_price_sqrt(netuid);
            let current_price_after =
                (sqrt_current_price_after * sqrt_current_price_after).to_num::<f64>();
            match order_type {
                OrderType::Buy => assert!(current_price_after > current_price),
                OrderType::Sell => assert!(current_price_after < current_price),
            }
        });

        // Current price shouldn't be much different from the original
        let sqrt_current_price_after = Pallet::<Test>::current_price_sqrt(netuid);
        let current_price_after =
            (sqrt_current_price_after * sqrt_current_price_after).to_num::<f64>();
        assert_abs_diff_eq!(
            current_price,
            current_price_after,
            epsilon = current_price / 10.
        )
    });
}

// cargo test --package pallet-subtensor-swap --lib -- pallet::impls::tests::test_swap_precision_edge_case --exact --show-output
#[test]
fn test_swap_precision_edge_case() {
    new_test_ext().execute_with(|| {
        let netuid = NetUid::from(123); // 123 is netuid with low edge case liquidity
        let order_type = OrderType::Sell;
        let liquidity = 1_000_000_000_000_000_000;
        let tick_low = TickIndex::MIN;

        let sqrt_limit_price: SqrtPrice = tick_low.try_to_sqrt_price().unwrap();

        // Setup swap
        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        // Swap
        let swap_result =
            Pallet::<Test>::do_swap(netuid, order_type, liquidity, sqrt_limit_price, false, true)
                .unwrap();

        assert!(swap_result.amount_paid_out > 0);
    });
}

// cargo test --package pallet-subtensor-swap --lib -- pallet::impls::tests::test_price_tick_price_roundtrip --exact --show-output
#[test]
fn test_price_tick_price_roundtrip() {
    new_test_ext().execute_with(|| {
        let netuid = NetUid::from(1);

        // Setup swap
        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        let current_price = SqrtPrice::from_num(0.500_000_512_192_122_7);
        let tick = TickIndex::try_from_sqrt_price(current_price).unwrap();

        let round_trip_price = TickIndex::try_to_sqrt_price(&tick).unwrap();
        assert!(round_trip_price <= current_price);

        let roundtrip_tick = TickIndex::try_from_sqrt_price(round_trip_price).unwrap();
        assert!(tick == roundtrip_tick);
    });
}

#[test]
fn test_convert_deltas() {
    new_test_ext().execute_with(|| {
        let netuid = NetUid::from(1);
        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        for (sqrt_price, delta_in, expected_buy, expected_sell) in [
            (SqrtPrice::from_num(1.5), 1, 0, 2),
            (SqrtPrice::from_num(1.5), 10000, 4444, 22500),
            (SqrtPrice::from_num(1.5), 1000000, 444444, 2250000),
            (
                SqrtPrice::from_num(1.5),
                u64::MAX,
                2000000000000,
                3000000000000,
            ),
            (
                TickIndex::MIN.as_sqrt_price_bounded(),
                1,
                18406523739291577836,
                465,
            ),
            (TickIndex::MIN.as_sqrt_price_bounded(), 10000, u64::MAX, 465),
            (
                TickIndex::MIN.as_sqrt_price_bounded(),
                1000000,
                u64::MAX,
                465,
            ),
            (
                TickIndex::MIN.as_sqrt_price_bounded(),
                u64::MAX,
                u64::MAX,
                464,
            ),
            (
                TickIndex::MAX.as_sqrt_price_bounded(),
                1,
                0,
                18406523745214495085,
            ),
            (TickIndex::MAX.as_sqrt_price_bounded(), 10000, 0, u64::MAX),
            (TickIndex::MAX.as_sqrt_price_bounded(), 1000000, 0, u64::MAX),
            (
                TickIndex::MAX.as_sqrt_price_bounded(),
                u64::MAX,
                2000000000000,
                u64::MAX,
            ),
        ] {
            {
                AlphaSqrtPrice::<Test>::insert(netuid, sqrt_price);

                assert_abs_diff_eq!(
                    Pallet::<Test>::convert_deltas(netuid, OrderType::Sell, delta_in),
                    expected_sell,
                    epsilon = 2
                );
                assert_abs_diff_eq!(
                    Pallet::<Test>::convert_deltas(netuid, OrderType::Buy, delta_in),
                    expected_buy,
                    epsilon = 2
                );
            }
        }
    });
}

#[test]
fn test_user_liquidity_disabled() {
    new_test_ext().execute_with(|| {
        // Use a netuid above 100 since our mock enables liquidity for 0-100
        let netuid = NetUid::from(101);
        let tick_low = TickIndex::new_unchecked(-1000);
        let tick_high = TickIndex::new_unchecked(1000);
        let position_id = PositionId::from(1);
        let liquidity = 1_000_000_000;
        let liquidity_delta = 500_000_000;

        assert!(!EnabledUserLiquidity::<Test>::get(netuid));

        assert_noop!(
            Swap::do_add_liquidity(
                netuid,
                &OK_COLDKEY_ACCOUNT_ID,
                &OK_HOTKEY_ACCOUNT_ID,
                tick_low,
                tick_high,
                liquidity
            ),
            Error::<Test>::UserLiquidityDisabled
        );

        assert_noop!(
            Swap::do_remove_liquidity(netuid, &OK_COLDKEY_ACCOUNT_ID, position_id),
            Error::<Test>::LiquidityNotFound
        );

        assert_noop!(
            Swap::modify_position(
                RuntimeOrigin::signed(OK_COLDKEY_ACCOUNT_ID),
                OK_HOTKEY_ACCOUNT_ID,
                netuid,
                position_id,
                liquidity_delta
            ),
            Error::<Test>::UserLiquidityDisabled
        );

        assert_ok!(Swap::toggle_user_liquidity(
            RuntimeOrigin::root(),
            netuid,
            true
        ));

        let position_id = Swap::do_add_liquidity(
            netuid,
            &OK_COLDKEY_ACCOUNT_ID,
            &OK_HOTKEY_ACCOUNT_ID,
            tick_low,
            tick_high,
            liquidity,
        )
        .unwrap()
        .0;

        assert_ok!(Swap::do_modify_position(
            netuid.into(),
            &OK_COLDKEY_ACCOUNT_ID,
            &OK_HOTKEY_ACCOUNT_ID,
            position_id,
            liquidity_delta,
        ));

        assert_ok!(Swap::do_remove_liquidity(
            netuid,
            &OK_COLDKEY_ACCOUNT_ID,
            position_id,
        ));
    });
}

/// Test correctness of swap fees:
///   - Fees are distribued to (concentrated) liquidity providers
///
#[test]
fn test_swap_fee_correctness() {
    new_test_ext().execute_with(|| {
        let min_price = tick_to_price(TickIndex::MIN);
        let max_price = tick_to_price(TickIndex::MAX);
        let netuid = NetUid::from(1);

        // Provide very spread liquidity at the range from min to max that matches protocol liquidity
        let liquidity = 2_000_000_000_000_u64; // 1x of protocol liquidity

        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        // Calculate ticks
        let tick_low = price_to_tick(min_price);
        let tick_high = price_to_tick(max_price);

        // Add user liquidity
        let (position_id, _tao, _alpha) = Pallet::<Test>::do_add_liquidity(
            netuid,
            &OK_COLDKEY_ACCOUNT_ID,
            &OK_HOTKEY_ACCOUNT_ID,
            tick_low,
            tick_high,
            liquidity,
        )
        .unwrap();

        // Swap buy and swap sell
        Pallet::<Test>::do_swap(
            netuid,
            OrderType::Buy,
            liquidity / 10,
            u64::MAX.into(),
            false,
            false,
        )
        .unwrap();
        Pallet::<Test>::do_swap(
            netuid,
            OrderType::Sell,
            liquidity / 10,
            0_u64.into(),
            false,
            false,
        )
        .unwrap();

        // Get user position
        let mut position =
            Positions::<Test>::get((netuid, OK_COLDKEY_ACCOUNT_ID, position_id)).unwrap();
        assert_eq!(position.liquidity, liquidity);
        assert_eq!(position.tick_low, tick_low);
        assert_eq!(position.tick_high, tick_high);

        // Check that 50% of fees were credited to the position
        let fee_rate = FeeRate::<Test>::get(NetUid::from(netuid)) as f64 / u16::MAX as f64;
        let (actual_fee_tao, actual_fee_alpha) = position.collect_fees();
        let expected_fee = (fee_rate * (liquidity / 10) as f64 * 0.5) as u64;

        assert_abs_diff_eq!(actual_fee_tao, expected_fee, epsilon = 1,);
        assert_abs_diff_eq!(actual_fee_alpha, expected_fee, epsilon = 1,);
    });
}

#[test]
fn test_current_liquidity_updates() {
    let netuid = NetUid::from(1);
    let liquidity = 1_000_000_000;

    // Get current price
    let (current_price, current_price_low, current_price_high) =
        new_test_ext().execute_with(|| {
            assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));
            let sqrt_current_price = AlphaSqrtPrice::<Test>::get(netuid);
            let current_price = (sqrt_current_price * sqrt_current_price).to_num::<f64>();
            let (current_price_low, current_price_high) = get_ticked_prices_around_current_price();
            (current_price, current_price_low, current_price_high)
        });

    // Test case: (price_low, price_high, expect_to_update)
    [
        // Current price is out of position range (lower), no current lq update
        (current_price * 2., current_price * 3., false),
        // Current price is out of position range (higher), no current lq update
        (current_price / 3., current_price / 2., false),
        // Current price is just below position range, no current lq update
        (current_price_high, current_price * 3., false),
        // Position lower edge is just below the current price, current lq updates
        (current_price_low, current_price * 3., true),
        // Current price is exactly at lower edge of position range, current lq updates
        (current_price, current_price * 3., true),
        // Current price is exactly at higher edge of position range, no current lq update
        (current_price / 2., current_price, false),
    ]
    .into_iter()
    .for_each(|(price_low, price_high, expect_to_update)| {
        new_test_ext().execute_with(|| {
            assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

            // Calculate ticks (assuming tick math is tested separately)
            let tick_low = price_to_tick(price_low);
            let tick_high = price_to_tick(price_high);
            let liquidity_before = CurrentLiquidity::<Test>::get(netuid);

            // Add liquidity
            assert_ok!(Pallet::<Test>::do_add_liquidity(
                netuid,
                &OK_COLDKEY_ACCOUNT_ID,
                &OK_HOTKEY_ACCOUNT_ID,
                tick_low,
                tick_high,
                liquidity,
            ));

            // Current liquidity is updated only when price range includes the current price
            let expected_liquidity = if (price_high > current_price) && (price_low <= current_price)
            {
                assert!(expect_to_update);
                liquidity_before + liquidity
            } else {
                assert!(!expect_to_update);
                liquidity_before
            };

            assert_eq!(CurrentLiquidity::<Test>::get(netuid), expected_liquidity)
        });
    });
}

#[test]
fn test_rollback_works() {
    new_test_ext().execute_with(|| {
        let netuid = NetUid::from(1);

        assert_eq!(
            Pallet::<Test>::do_swap(
                netuid,
                OrderType::Buy,
                1_000_000,
                u64::MAX.into(),
                false,
                true
            )
            .unwrap(),
            Pallet::<Test>::do_swap(
                netuid,
                OrderType::Buy,
                1_000_000,
                u64::MAX.into(),
                false,
                false
            )
            .unwrap()
        );
    })
}

/// Test correctness of swap fees:
///   - New LP is not eligible to previously accrued fees
///
/// cargo test --package pallet-subtensor-swap --lib -- pallet::tests::test_new_lp_doesnt_get_old_fees --exact --show-output
#[test]
fn test_new_lp_doesnt_get_old_fees() {
    new_test_ext().execute_with(|| {
        let min_price = tick_to_price(TickIndex::MIN);
        let max_price = tick_to_price(TickIndex::MAX);
        let netuid = NetUid::from(1);

        // Provide very spread liquidity at the range from min to max that matches protocol liquidity
        let liquidity = 2_000_000_000_000_u64; // 1x of protocol liquidity

        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        // Calculate ticks
        let tick_low = price_to_tick(min_price);
        let tick_high = price_to_tick(max_price);

        // Add user liquidity
        Pallet::<Test>::do_add_liquidity(
            netuid,
            &OK_COLDKEY_ACCOUNT_ID,
            &OK_HOTKEY_ACCOUNT_ID,
            tick_low,
            tick_high,
            liquidity,
        )
        .unwrap();

        // Swap buy and swap sell
        Pallet::<Test>::do_swap(
            netuid,
            OrderType::Buy,
            liquidity / 10,
            u64::MAX.into(),
            false,
            false,
        )
        .unwrap();
        Pallet::<Test>::do_swap(
            netuid,
            OrderType::Sell,
            liquidity / 10,
            0_u64.into(),
            false,
            false,
        )
        .unwrap();

        // Add liquidity from a different user to a new tick
        let (position_id_2, _tao, _alpha) = Pallet::<Test>::do_add_liquidity(
            netuid,
            &OK_COLDKEY_ACCOUNT_ID_2,
            &OK_HOTKEY_ACCOUNT_ID_2,
            tick_low.next().unwrap(),
            tick_high.prev().unwrap(),
            liquidity,
        )
        .unwrap();

        // Get user position
        let mut position =
            Positions::<Test>::get((netuid, OK_COLDKEY_ACCOUNT_ID_2, position_id_2)).unwrap();
        assert_eq!(position.liquidity, liquidity);
        assert_eq!(position.tick_low, tick_low.next().unwrap());
        assert_eq!(position.tick_high, tick_high.prev().unwrap());

        // Check that collected fees are 0
        let (actual_fee_tao, actual_fee_alpha) = position.collect_fees();
        assert_abs_diff_eq!(actual_fee_tao, 0, epsilon = 1);
        assert_abs_diff_eq!(actual_fee_alpha, 0, epsilon = 1);
    });
}

fn bbox(t: U64F64, a: U64F64, b: U64F64) -> U64F64 {
    if t < a {
        a
    } else if t > b {
        b
    } else {
        t
    }
}

fn print_current_price(netuid: NetUid) {
    let current_sqrt_price = Pallet::<Test>::current_price_sqrt(netuid).to_num::<f64>();
    let current_price = current_sqrt_price * current_sqrt_price;
    log::trace!("Current price: {:.6}", current_price);
}

/// RUST_LOG=pallet_subtensor_swap=trace cargo test --package pallet-subtensor-swap --lib -- pallet::tests::test_wrapping_fees --exact --show-output --nocapture
#[test]
fn test_wrapping_fees() {
    new_test_ext().execute_with(|| {
        let netuid = NetUid::from(WRAPPING_FEES_NETUID);
        let position_1_low_price = 0.20;
        let position_1_high_price = 0.255;
        let position_2_low_price = 0.255;
        let position_2_high_price = 0.257;
        assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

        Pallet::<Test>::do_add_liquidity(
            netuid,
            &OK_COLDKEY_ACCOUNT_ID_RICH,
            &OK_COLDKEY_ACCOUNT_ID_RICH,
            price_to_tick(position_1_low_price),
            price_to_tick(position_1_high_price),
            1_000_000_000_u64,
        )
        .unwrap();

        print_current_price(netuid);

        let swap_amt = 800_000_000_u64;
        let order_type = OrderType::Sell;
        let sqrt_limit_price = SqrtPrice::from_num(0.000001);
        Pallet::<Test>::do_swap(netuid, order_type, swap_amt, sqrt_limit_price, false, false)
            .unwrap();

        let swap_amt = 1_850_000_000_u64;
        let order_type = OrderType::Buy;
        let sqrt_limit_price = SqrtPrice::from_num(1_000_000.0);

        print_current_price(netuid);

        Pallet::<Test>::do_swap(netuid, order_type, swap_amt, sqrt_limit_price, false, false)
            .unwrap();

        print_current_price(netuid);

        let add_liquidity_result = Pallet::<Test>::do_add_liquidity(
            netuid,
            &OK_COLDKEY_ACCOUNT_ID_RICH,
            &OK_COLDKEY_ACCOUNT_ID_RICH,
            price_to_tick(position_2_low_price),
            price_to_tick(position_2_high_price),
            1_000_000_000_u64,
        )
        .unwrap();

        let swap_amt = 1_800_000_000_u64;
        let order_type = OrderType::Sell;
        let sqrt_limit_price = SqrtPrice::from_num(0.000001);

        let initial_sqrt_price = Pallet::<Test>::current_price_sqrt(netuid);
        Pallet::<Test>::do_swap(netuid, order_type, swap_amt, sqrt_limit_price, false, false)
            .unwrap();
        let final_sqrt_price = Pallet::<Test>::current_price_sqrt(netuid);

        print_current_price(netuid);

        let mut position =
            Positions::<Test>::get((netuid, &OK_COLDKEY_ACCOUNT_ID_RICH, add_liquidity_result.0))
                .unwrap();

        let initial_box_price = bbox(
            initial_sqrt_price,
            position.tick_low.try_to_sqrt_price().unwrap(),
            position.tick_high.try_to_sqrt_price().unwrap(),
        );

        let final_box_price = bbox(
            final_sqrt_price,
            position.tick_low.try_to_sqrt_price().unwrap(),
            position.tick_high.try_to_sqrt_price().unwrap(),
        );

        let fee_rate = FeeRate::<Test>::get(netuid) as f64 / u16::MAX as f64;

        log::trace!("fee_rate: {:.6}", fee_rate);
        log::trace!("position.liquidity: {}", position.liquidity);
        log::trace!(
            "initial_box_price: {:.6}",
            initial_box_price.to_num::<f64>()
        );
        log::trace!("final_box_price: {:.6}", final_box_price.to_num::<f64>());

        let expected_fee_tao = ((fee_rate / (1.0 - fee_rate))
            * (position.liquidity as f64)
            * (final_box_price.to_num::<f64>() - initial_box_price.to_num::<f64>()))
            as u64;

        let expected_fee_alpha = ((fee_rate / (1.0 - fee_rate))
            * (position.liquidity as f64)
            * ((1.0 / final_box_price.to_num::<f64>()) - (1.0 / initial_box_price.to_num::<f64>())))
            as u64;

        log::trace!("Expected ALPHA fee: {:.6}", expected_fee_alpha as f64);

        let (fee_tao, fee_alpha) = position.collect_fees();

        log::trace!("Collected fees: TAO: {}, ALPHA: {}", fee_tao, fee_alpha);

        assert_abs_diff_eq!(fee_tao, expected_fee_tao, epsilon = 1);
        assert_abs_diff_eq!(fee_alpha, expected_fee_alpha, epsilon = 1);
    });
}

/// Test that price moves less with provided liquidity
/// cargo test --package pallet-subtensor-swap --lib -- pallet::tests::test_less_price_movement --exact --show-output
#[test]
fn test_less_price_movement() {
    let netuid = NetUid::from(1);
    let mut last_end_price = U96F32::from_num(0);
    let initial_stake_liquidity = 1_000_000_000;
    let swapped_liquidity = 1_000_000;

    // Test case is (order_type, provided_liquidity)
    // Testing algorithm:
    //   - Stake initial_stake_liquidity
    //   - Provide liquidity if iteration provides lq
    //   - Buy or sell
    //   - Save end price if iteration doesn't provide lq
    [
        (OrderType::Buy, 0_u64),
        (OrderType::Buy, 1_000_000_000_000_u64),
        (OrderType::Sell, 0_u64),
        (OrderType::Sell, 1_000_000_000_000_u64),
    ]
    .into_iter()
    .for_each(|(order_type, provided_liquidity)| {
        new_test_ext().execute_with(|| {
            // Setup swap
            assert_ok!(Pallet::<Test>::maybe_initialize_v3(netuid));

            // Buy Alpha
            assert_ok!(Pallet::<Test>::do_swap(
                netuid,
                OrderType::Buy,
                initial_stake_liquidity,
                SqrtPrice::from_num(10_000_000_000_u64),
                false,
                false
            ));

            // Get current price
            let start_price = Pallet::<Test>::current_price(netuid);

            // Add liquidity if this test iteration provides
            if provided_liquidity > 0 {
                let tick_low = price_to_tick(start_price.to_num::<f64>() * 0.5);
                let tick_high = price_to_tick(start_price.to_num::<f64>() * 1.5);
                assert_ok!(Pallet::<Test>::do_add_liquidity(
                    netuid,
                    &OK_COLDKEY_ACCOUNT_ID,
                    &OK_HOTKEY_ACCOUNT_ID,
                    tick_low,
                    tick_high,
                    provided_liquidity,
                ));
            }

            // Swap
            let sqrt_limit_price = if order_type == OrderType::Buy {
                SqrtPrice::from_num(1000.)
            } else {
                SqrtPrice::from_num(0.001)
            };
            assert_ok!(Pallet::<Test>::do_swap(
                netuid,
                order_type,
                swapped_liquidity,
                sqrt_limit_price,
                false,
                false
            ));

            let end_price = Pallet::<Test>::current_price(netuid);

            // Save end price if iteration doesn't provide or compare with previous end price if it does
            if provided_liquidity > 0 {
                if order_type == OrderType::Buy {
                    assert!(end_price < last_end_price);
                } else {
                    assert!(end_price > last_end_price);
                }
            } else {
                last_end_price = end_price;
            }
        });
    });
}
