#![allow(clippy::crate_in_macro_def)]
use frame_support::pallet_macros::pallet_section;

/// A [`pallet_section`] that defines the errors for a pallet.
/// This can later be imported into the pallet using [`import_section`].
#[pallet_section]
mod dispatches {
    use crate::subnets::leasing::SubnetLeasingWeightInfo;
    use frame_support::traits::schedule::DispatchTime;
    use frame_support::traits::schedule::v3::Anon as ScheduleAnon;
    use frame_system::pallet_prelude::BlockNumberFor;
    use sp_core::ecdsa::Signature;
    use sp_runtime::{Percent, traits::Saturating};

    use crate::MAX_CRV3_COMMIT_SIZE_BYTES;
    /// Dispatchable functions allow users to interact with the pallet and invoke state changes.
    /// These functions materialize as "extrinsics", which are often compared to transactions.
    /// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// --- Sets the caller weights for the incentive mechanism. The call can be
        /// made from the hotkey account so is potentially insecure, however, the damage
        /// of changing weights is minimal if caught early. This function includes all the
        /// checks that the passed weights meet the requirements. Stored as u16s they represent
        /// rational values in the range [0,1] which sum to 1 and can be interpreted as
        /// probabilities. The specific weights determine how inflation propagates outward
        /// from this peer.
        ///
        /// Note: The 16 bit integers weights should represent 1.0 as the max u16.
        /// However, the function normalizes all integers to u16_max anyway. This means that if the sum of all
        /// elements is larger or smaller than the amount of elements * u16_max, all elements
        /// will be corrected for this deviation.
        ///
        /// # Args:
        /// * `origin`: (<T as frame_system::Config>Origin):
        ///     - The caller, a hotkey who wishes to set their weights.
        ///
        /// * `netuid` (u16):
        /// 	- The network uid we are setting these weights on.
        ///
        /// * `dests` (Vec<u16>):
        /// 	- The edge endpoint for the weight, i.e. j for w_ij.
        ///
        /// * 'weights' (Vec<u16>):
        /// 	- The u16 integer encoded weights. Interpreted as rational
        /// 		values in the range [0,1]. They must sum to in32::MAX.
        ///
        /// * 'version_key' ( u64 ):
        /// 	- The network version key to check if the validator is up to date.
        ///
        /// # Event:
        /// * WeightsSet;
        /// 	- On successfully setting the weights on chain.
        ///
        /// # Raises:
        /// * 'SubNetworkDoesNotExist':
        /// 	- Attempting to set weights on a non-existent network.
        ///
        /// * 'NotRegistered':
        /// 	- Attempting to set weights from a non registered account.
        ///
        /// * 'WeightVecNotEqualSize':
        /// 	- Attempting to set weights with uids not of same length.
        ///
        /// * 'DuplicateUids':
        /// 	- Attempting to set weights with duplicate uids.
        ///
        ///     * 'UidsLengthExceedUidsInSubNet':
        /// 	- Attempting to set weights above the max allowed uids.
        ///
        /// * 'UidVecContainInvalidOne':
        /// 	- Attempting to set weights with invalid uids.
        ///
        /// * 'WeightVecLengthIsLow':
        /// 	- Attempting to set weights with fewer weights than min.
        ///
        /// * 'MaxWeightExceeded':
        /// 	- Attempting to set weights with max value exceeding limit.
        #[pallet::call_index(0)]
        #[pallet::weight((Weight::from_parts(20_730_000_000, 0)
        .saturating_add(T::DbWeight::get().reads(4111))
        .saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Normal, Pays::No))]
        pub fn set_weights(
            origin: OriginFor<T>,
            netuid: NetUid,
            dests: Vec<u16>,
            weights: Vec<u16>,
            version_key: u64,
        ) -> DispatchResult {
            if Self::get_commit_reveal_weights_enabled(netuid) {
                Err(Error::<T>::CommitRevealEnabled.into())
            } else {
                Self::do_set_weights(origin, netuid, dests, weights, version_key)
            }
        }

        /// --- Allows a hotkey to set weights for multiple netuids as a batch.
        ///
        /// # Args:
        /// * `origin`: (<T as frame_system::Config>Origin):
        ///     - The caller, a hotkey who wishes to set their weights.
        ///
        /// * `netuids` (Vec<Compact<u16>>):
        /// 	- The network uids we are setting these weights on.
        ///
        /// * `weights` (Vec<Vec<(Compact<u16>, Compact<u16>)>):
        /// 	- The weights to set for each network. [(uid, weight), ...]
        ///
        /// * `version_keys` (Vec<Compact<u64>>):
        /// 	- The network version keys to check if the validator is up to date.
        ///
        /// # Event:
        /// * WeightsSet;
        /// 	- On successfully setting the weights on chain.
        /// * BatchWeightsCompleted;
        /// 	- On success of the batch.
        /// * BatchCompletedWithErrors;
        /// 	- On failure of any of the weights in the batch.
        /// * BatchWeightItemFailed;
        /// 	- On failure for each failed item in the batch.
        ///
        #[pallet::call_index(80)]
        #[pallet::weight((Weight::from_parts(105_100_000, 0)
        .saturating_add(T::DbWeight::get().reads(14))
        .saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Normal, Pays::No))]
        pub fn batch_set_weights(
            origin: OriginFor<T>,
            netuids: Vec<Compact<NetUid>>,
            weights: Vec<Vec<(Compact<u16>, Compact<u16>)>>,
            version_keys: Vec<Compact<u64>>,
        ) -> DispatchResult {
            Self::do_batch_set_weights(origin, netuids, weights, version_keys)
        }

        /// ---- Used to commit a hash of your weight values to later be revealed.
        ///
        /// # Args:
        /// * `origin`: (`<T as frame_system::Config>::RuntimeOrigin`):
        ///   - The signature of the committing hotkey.
        ///
        /// * `netuid` (`u16`):
        ///   - The u16 network identifier.
        ///
        /// * `commit_hash` (`H256`):
        ///   - The hash representing the committed weights.
        ///
        /// # Raises:
        /// * `CommitRevealDisabled`:
        ///   - Attempting to commit when the commit-reveal mechanism is disabled.
        ///
        /// * `TooManyUnrevealedCommits`:
        ///   - Attempting to commit when the user has more than the allowed limit of unrevealed commits.
        ///
        #[pallet::call_index(96)]
        #[pallet::weight((Weight::from_parts(72_300_000, 0)
		.saturating_add(T::DbWeight::get().reads(7))
		.saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Normal, Pays::No))]
        pub fn commit_weights(
            origin: T::RuntimeOrigin,
            netuid: NetUid,
            commit_hash: H256,
        ) -> DispatchResult {
            Self::do_commit_weights(origin, netuid, commit_hash)
        }

        /// --- Allows a hotkey to commit weight hashes for multiple netuids as a batch.
        ///
        /// # Args:
        /// * `origin`: (<T as frame_system::Config>Origin):
        ///     - The caller, a hotkey who wishes to set their weights.
        ///
        /// * `netuids` (Vec<Compact<u16>>):
        /// 	- The network uids we are setting these weights on.
        ///
        /// * `commit_hashes` (Vec<H256>):
        /// 	- The commit hashes to commit.
        ///
        /// # Event:
        /// * WeightsSet;
        /// 	- On successfully setting the weights on chain.
        /// * BatchWeightsCompleted;
        /// 	- On success of the batch.
        /// * BatchCompletedWithErrors;
        /// 	- On failure of any of the weights in the batch.
        /// * BatchWeightItemFailed;
        /// 	- On failure for each failed item in the batch.
        ///
        #[pallet::call_index(100)]
        #[pallet::weight((Weight::from_parts(89_380_000, 0)
        .saturating_add(T::DbWeight::get().reads(8))
        .saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Normal, Pays::No))]
        pub fn batch_commit_weights(
            origin: OriginFor<T>,
            netuids: Vec<Compact<NetUid>>,
            commit_hashes: Vec<H256>,
        ) -> DispatchResult {
            Self::do_batch_commit_weights(origin, netuids, commit_hashes)
        }

        /// ---- Used to reveal the weights for a previously committed hash.
        ///
        /// # Args:
        /// * `origin`: (`<T as frame_system::Config>::RuntimeOrigin`):
        ///   - The signature of the revealing hotkey.
        ///
        /// * `netuid` (`u16`):
        ///   - The u16 network identifier.
        ///
        /// * `uids` (`Vec<u16>`):
        ///   - The uids for the weights being revealed.
        ///
        /// * `values` (`Vec<u16>`):
        ///   - The values of the weights being revealed.
        ///
        /// * `salt` (`Vec<u16>`):
        ///   - The salt used to generate the commit hash.
        ///
        /// * `version_key` (`u64`):
        ///   - The network version key.
        ///
        /// # Raises:
        /// * `CommitRevealDisabled`:
        ///   - Attempting to reveal weights when the commit-reveal mechanism is disabled.
        ///
        /// * `NoWeightsCommitFound`:
        ///   - Attempting to reveal weights without an existing commit.
        ///
        /// * `ExpiredWeightCommit`:
        ///   - Attempting to reveal a weight commit that has expired.
        ///
        /// * `RevealTooEarly`:
        ///   - Attempting to reveal weights outside the valid reveal period.
        ///
        /// * `InvalidRevealCommitHashNotMatch`:
        ///   - The revealed hash does not match any committed hash.
        ///
        #[pallet::call_index(97)]
        #[pallet::weight((Weight::from_parts(122_000_000, 0)
		.saturating_add(T::DbWeight::get().reads(16))
		.saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Normal, Pays::No))]
        pub fn reveal_weights(
            origin: T::RuntimeOrigin,
            netuid: NetUid,
            uids: Vec<u16>,
            values: Vec<u16>,
            salt: Vec<u16>,
            version_key: u64,
        ) -> DispatchResult {
            Self::do_reveal_weights(origin, netuid, uids, values, salt, version_key)
        }

        /// ---- Used to commit encrypted commit-reveal v3 weight values to later be revealed.
        ///
        /// # Args:
        /// * `origin`: (`<T as frame_system::Config>::RuntimeOrigin`):
        ///   - The committing hotkey.
        ///
        /// * `netuid` (`u16`):
        ///   - The u16 network identifier.
        ///
        /// * `commit` (`Vec<u8>`):
        ///   - The encrypted compressed commit.
        ///     The steps for this are:
        ///     1. Instantiate [`WeightsTlockPayload`]
        ///     2. Serialize it using the `parity_scale_codec::Encode` trait
        ///     3. Encrypt it following the steps (here)[https://github.com/ideal-lab5/tle/blob/f8e6019f0fb02c380ebfa6b30efb61786dede07b/timelock/src/tlock.rs#L283-L336]
        ///        to produce a [`TLECiphertext<TinyBLS381>`] type.
        ///     4. Serialize and compress using the `ark-serialize` `CanonicalSerialize` trait.
        ///
        /// * reveal_round (`u64`):
        ///    - The drand reveal round which will be avaliable during epoch `n+1` from the current
        ///      epoch.
        ///
        /// # Raises:
        /// * `CommitRevealV3Disabled`:
        ///   - Attempting to commit when the commit-reveal mechanism is disabled.
        ///
        /// * `TooManyUnrevealedCommits`:
        ///   - Attempting to commit when the user has more than the allowed limit of unrevealed commits.
        ///
        #[pallet::call_index(99)]
        #[pallet::weight((Weight::from_parts(73_750_000, 0)
		.saturating_add(T::DbWeight::get().reads(6_u64))
		.saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Normal, Pays::No))]
        pub fn commit_crv3_weights(
            origin: T::RuntimeOrigin,
            netuid: NetUid,
            commit: BoundedVec<u8, ConstU32<MAX_CRV3_COMMIT_SIZE_BYTES>>,
            reveal_round: u64,
        ) -> DispatchResult {
            Self::do_commit_crv3_weights(origin, netuid, commit, reveal_round)
        }

        /// ---- The implementation for batch revealing committed weights.
        ///
        /// # Args:
        /// * `origin`: (`<T as frame_system::Config>::RuntimeOrigin`):
        ///   - The signature of the revealing hotkey.
        ///
        /// * `netuid` (`u16`):
        ///   - The u16 network identifier.
        ///
        /// * `uids_list` (`Vec<Vec<u16>>`):
        ///   - A list of uids for each set of weights being revealed.
        ///
        /// * `values_list` (`Vec<Vec<u16>>`):
        ///   - A list of values for each set of weights being revealed.
        ///
        /// * `salts_list` (`Vec<Vec<u16>>`):
        ///   - A list of salts used to generate the commit hashes.
        ///
        /// * `version_keys` (`Vec<u64>`):
        ///   - A list of network version keys.
        ///
        /// # Raises:
        /// * `CommitRevealDisabled`:
        ///   - Attempting to reveal weights when the commit-reveal mechanism is disabled.
        ///
        /// * `NoWeightsCommitFound`:
        ///   - Attempting to reveal weights without an existing commit.
        ///
        /// * `ExpiredWeightCommit`:
        ///   - Attempting to reveal a weight commit that has expired.
        ///
        /// * `RevealTooEarly`:
        ///   - Attempting to reveal weights outside the valid reveal period.
        ///
        /// * `InvalidRevealCommitHashNotMatch`:
        ///   - The revealed hash does not match any committed hash.
        ///
        /// * `InvalidInputLengths`:
        ///   - The input vectors are of mismatched lengths.
        #[pallet::call_index(98)]
        #[pallet::weight((Weight::from_parts(420_500_000, 0)
		.saturating_add(T::DbWeight::get().reads(16))
		.saturating_add(T::DbWeight::get().writes(2_u64)), DispatchClass::Normal, Pays::No))]
        pub fn batch_reveal_weights(
            origin: T::RuntimeOrigin,
            netuid: NetUid,
            uids_list: Vec<Vec<u16>>,
            values_list: Vec<Vec<u16>>,
            salts_list: Vec<Vec<u16>>,
            version_keys: Vec<u64>,
        ) -> DispatchResult {
            Self::do_batch_reveal_weights(
                origin,
                netuid,
                uids_list,
                values_list,
                salts_list,
                version_keys,
            )
        }

        /// # Args:
        /// * `origin`: (<T as frame_system::Config>Origin):
        /// 	- The caller, a hotkey who wishes to set their weights.
        ///
        /// * `netuid` (u16):
        /// 	- The network uid we are setting these weights on.
        ///
        /// * `hotkey` (T::AccountId):
        /// 	- The hotkey associated with the operation and the calling coldkey.
        ///
        /// * `dests` (Vec<u16>):
        /// 	- The edge endpoint for the weight, i.e. j for w_ij.
        ///
        /// * 'weights' (Vec<u16>):
        /// 	- The u16 integer encoded weights. Interpreted as rational
        /// 		values in the range [0,1]. They must sum to in32::MAX.
        ///
        /// * 'version_key' ( u64 ):
        /// 	- The network version key to check if the validator is up to date.
        ///
        /// # Event:
        ///
        /// * WeightsSet;
        /// 	- On successfully setting the weights on chain.
        ///
        /// # Raises:
        ///
        /// * NonAssociatedColdKey;
        /// 	- Attempting to set weights on a non-associated cold key.
        ///
        /// * 'SubNetworkDoesNotExist':
        /// 	- Attempting to set weights on a non-existent network.
        ///
        /// * 'NotRootSubnet':
        /// 	- Attempting to set weights on a subnet that is not the root network.
        ///
        /// * 'WeightVecNotEqualSize':
        /// 	- Attempting to set weights with uids not of same length.
        ///
        /// * 'UidVecContainInvalidOne':
        /// 	- Attempting to set weights with invalid uids.
        ///
        /// * 'NotRegistered':
        /// 	- Attempting to set weights from a non registered account.
        ///
        /// * 'WeightVecLengthIsLow':
        /// 	- Attempting to set weights with fewer weights than min.
        ///
        ///  * 'IncorrectWeightVersionKey':
        ///      - Attempting to set weights with the incorrect network version key.
        ///
        ///  * 'SettingWeightsTooFast':
        ///      - Attempting to set weights too fast.
        ///
        /// * 'WeightVecLengthIsLow':
        /// 	- Attempting to set weights with fewer weights than min.
        ///
        /// * 'MaxWeightExceeded':
        /// 	- Attempting to set weights with max value exceeding limit.
        ///
        #[pallet::call_index(8)]
        #[pallet::weight((Weight::from_parts(3_757_000, 0)
		.saturating_add(T::DbWeight::get().reads(0_u64))
		.saturating_add(T::DbWeight::get().writes(0_u64)), DispatchClass::Normal, Pays::No))]
        pub fn set_tao_weights(
            _origin: OriginFor<T>,
            _netuid: NetUid,
            _hotkey: T::AccountId,
            _dests: Vec<u16>,
            _weights: Vec<u16>,
            _version_key: u64,
        ) -> DispatchResult {
            // DEPRECATED
            // Self::do_set_root_weights(origin, netuid, hotkey, dests, weights, version_key)
            // Self::do_set_tao_weights(origin, netuid, hotkey, dests, weights, version_key)
            Ok(())
        }

        /// --- Sets the key as a delegate.
        ///
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the caller's coldkey.
        ///
        /// * 'hotkey' (T::AccountId):
        /// 	- The hotkey we are delegating (must be owned by the coldkey.)
        ///
        /// * 'take' (u64):
        /// 	- The stake proportion that this hotkey takes from delegations.
        ///
        /// # Event:
        /// * DelegateAdded;
        /// 	- On successfully setting a hotkey as a delegate.
        ///
        /// # Raises:
        /// * 'NotRegistered':
        /// 	- The hotkey we are delegating is not registered on the network.
        ///
        /// * 'NonAssociatedColdKey':
        /// 	- The hotkey we are delegating is not owned by the calling coldket.
        ///
        #[pallet::call_index(1)]
        #[pallet::weight((Weight::from_parts(3_657_000, 0)
		.saturating_add(T::DbWeight::get().reads(0))
		.saturating_add(T::DbWeight::get().writes(0)), DispatchClass::Normal, Pays::Yes))]
        pub fn become_delegate(_origin: OriginFor<T>, _hotkey: T::AccountId) -> DispatchResult {
            // DEPRECATED
            // Self::do_become_delegate(origin, hotkey, Self::get_default_delegate_take())

            Ok(())
        }

        /// --- Allows delegates to decrease its take value.
        ///
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>::Origin):
        /// 	- The signature of the caller's coldkey.
        ///
        /// * 'hotkey' (T::AccountId):
        /// 	- The hotkey we are delegating (must be owned by the coldkey.)
        ///
        /// * 'netuid' (u16):
        /// 	- Subnet ID to decrease take for
        ///
        /// * 'take' (u16):
        /// 	- The new stake proportion that this hotkey takes from delegations.
        ///        The new value can be between 0 and 11_796 and should be strictly
        ///        lower than the previous value. It T is the new value (rational number),
        ///        the the parameter is calculated as [65535 * T]. For example, 1% would be
        ///        [0.01 * 65535] = [655.35] = 655
        ///
        /// # Event:
        /// * TakeDecreased;
        /// 	- On successfully setting a decreased take for this hotkey.
        ///
        /// # Raises:
        /// * 'NotRegistered':
        /// 	- The hotkey we are delegating is not registered on the network.
        ///
        /// * 'NonAssociatedColdKey':
        /// 	- The hotkey we are delegating is not owned by the calling coldkey.
        ///
        /// * 'DelegateTakeTooLow':
        /// 	- The delegate is setting a take which is not lower than the previous.
        ///
        #[pallet::call_index(65)]
        #[pallet::weight((Weight::from_parts(37_380_000, 0)
		.saturating_add(T::DbWeight::get().reads(3))
		.saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Normal, Pays::No))]
        pub fn decrease_take(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            take: u16,
        ) -> DispatchResult {
            Self::do_decrease_take(origin, hotkey, take)
        }

        /// --- Allows delegates to increase its take value. This call is rate-limited.
        ///
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>::Origin):
        /// 	- The signature of the caller's coldkey.
        ///
        /// * 'hotkey' (T::AccountId):
        /// 	- The hotkey we are delegating (must be owned by the coldkey.)
        ///
        /// * 'take' (u16):
        /// 	- The new stake proportion that this hotkey takes from delegations.
        ///        The new value can be between 0 and 11_796 and should be strictly
        ///        greater than the previous value. T is the new value (rational number),
        ///        the the parameter is calculated as [65535 * T]. For example, 1% would be
        ///        [0.01 * 65535] = [655.35] = 655
        ///
        /// # Event:
        /// * TakeIncreased;
        /// 	- On successfully setting a increased take for this hotkey.
        ///
        /// # Raises:
        /// * 'NotRegistered':
        /// 	- The hotkey we are delegating is not registered on the network.
        ///
        /// * 'NonAssociatedColdKey':
        /// 	- The hotkey we are delegating is not owned by the calling coldkey.
        ///
        /// * 'DelegateTakeTooHigh':
        /// 	- The delegate is setting a take which is not greater than the previous.
        ///
        #[pallet::call_index(66)]
        #[pallet::weight((Weight::from_parts(44_630_000, 0)
		.saturating_add(T::DbWeight::get().reads(5))
		.saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Normal, Pays::No))]
        pub fn increase_take(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            take: u16,
        ) -> DispatchResult {
            Self::do_increase_take(origin, hotkey, take)
        }

        /// --- Adds stake to a hotkey. The call is made from a coldkey account.
        /// This delegates stake to the hotkey.
        ///
        /// Note: the coldkey account may own the hotkey, in which case they are
        /// delegating to themselves.
        ///
        /// # Args:
        ///  * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the caller's coldkey.
        ///
        ///  * 'hotkey' (T::AccountId):
        /// 	- The associated hotkey account.
        ///
        /// * 'netuid' (u16):
        ///     - Subnetwork UID
        ///
        ///  * 'amount_staked' (u64):
        /// 	- The amount of stake to be added to the hotkey staking account.
        ///
        /// # Event:
        ///  * StakeAdded;
        /// 	- On the successfully adding stake to a global account.
        ///
        /// # Raises:
        ///  * 'NotEnoughBalanceToStake':
        /// 	- Not enough balance on the coldkey to add onto the global account.
        ///
        ///  * 'NonAssociatedColdKey':
        /// 	- The calling coldkey is not associated with this hotkey.
        ///
        ///  * 'BalanceWithdrawalError':
        ///  	- Errors stemming from transaction pallet.
        ///
        #[pallet::call_index(2)]
        #[pallet::weight((Weight::from_parts(345_500_000, 0)
		.saturating_add(T::DbWeight::get().reads(26))
		.saturating_add(T::DbWeight::get().writes(15)), DispatchClass::Normal, Pays::Yes))]
        pub fn add_stake(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            netuid: NetUid,
            amount_staked: u64,
        ) -> DispatchResult {
            Self::do_add_stake(origin, hotkey, netuid, amount_staked)
        }

        /// Remove stake from the staking account. The call must be made
        /// from the coldkey account attached to the neuron metadata. Only this key
        /// has permission to make staking and unstaking requests.
        ///
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the caller's coldkey.
        ///
        /// * 'hotkey' (T::AccountId):
        /// 	- The associated hotkey account.
        ///
        /// * 'netuid' (u16):
        ///     - Subnetwork UID
        ///
        /// * 'amount_unstaked' (u64):
        /// 	- The amount of stake to be added to the hotkey staking account.
        ///
        /// # Event:
        /// * StakeRemoved;
        /// 	- On the successfully removing stake from the hotkey account.
        ///
        /// # Raises:
        /// * 'NotRegistered':
        /// 	- Thrown if the account we are attempting to unstake from is non existent.
        ///
        /// * 'NonAssociatedColdKey':
        /// 	- Thrown if the coldkey does not own the hotkey we are unstaking from.
        ///
        /// * 'NotEnoughStakeToWithdraw':
        /// 	- Thrown if there is not enough stake on the hotkey to withdwraw this amount.
        ///
        #[pallet::call_index(3)]
        #[pallet::weight((Weight::from_parts(196_800_000, 0)
		.saturating_add(T::DbWeight::get().reads(19))
		.saturating_add(T::DbWeight::get().writes(10)), DispatchClass::Normal, Pays::Yes))]
        pub fn remove_stake(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            netuid: NetUid,
            amount_unstaked: AlphaCurrency,
        ) -> DispatchResult {
            Self::do_remove_stake(origin, hotkey, netuid, amount_unstaked)
        }

        /// Serves or updates axon /prometheus information for the neuron associated with the caller. If the caller is
        /// already registered the metadata is updated. If the caller is not registered this call throws NotRegistered.
        ///
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the caller.
        ///
        /// * 'netuid' (u16):
        /// 	- The u16 network identifier.
        ///
        /// * 'version' (u64):
        /// 	- The bittensor version identifier.
        ///
        /// * 'ip' (u64):
        /// 	- The endpoint ip information as a u128 encoded integer.
        ///
        /// * 'port' (u16):
        /// 	- The endpoint port information as a u16 encoded integer.
        ///
        /// * 'ip_type' (u8):
        /// 	- The endpoint ip version as a u8, 4 or 6.
        ///
        /// * 'protocol' (u8):
        /// 	- UDP:1 or TCP:0
        ///
        /// * 'placeholder1' (u8):
        /// 	- Placeholder for further extra params.
        ///
        /// * 'placeholder2' (u8):
        /// 	- Placeholder for further extra params.
        ///
        /// # Event:
        /// * AxonServed;
        /// 	- On successfully serving the axon info.
        ///
        /// # Raises:
        /// * 'SubNetworkDoesNotExist':
        /// 	- Attempting to set weights on a non-existent network.
        ///
        /// * 'NotRegistered':
        /// 	- Attempting to set weights from a non registered account.
        ///
        /// * 'InvalidIpType':
        /// 	- The ip type is not 4 or 6.
        ///
        /// * 'InvalidIpAddress':
        /// 	- The numerically encoded ip address does not resolve to a proper ip.
        ///
        /// * 'ServingRateLimitExceeded':
        /// 	- Attempting to set prometheus information withing the rate limit min.
        ///
        #[pallet::call_index(4)]
        #[pallet::weight((Weight::from_parts(35_670_000, 0)
		.saturating_add(T::DbWeight::get().reads(4))
		.saturating_add(T::DbWeight::get().writes(1)), DispatchClass::Normal, Pays::No))]
        pub fn serve_axon(
            origin: OriginFor<T>,
            netuid: NetUid,
            version: u32,
            ip: u128,
            port: u16,
            ip_type: u8,
            protocol: u8,
            placeholder1: u8,
            placeholder2: u8,
        ) -> DispatchResult {
            Self::do_serve_axon(
                origin,
                netuid,
                version,
                ip,
                port,
                ip_type,
                protocol,
                placeholder1,
                placeholder2,
                None,
            )
        }

        /// Same as `serve_axon` but takes a certificate as an extra optional argument.
        /// Serves or updates axon /prometheus information for the neuron associated with the caller. If the caller is
        /// already registered the metadata is updated. If the caller is not registered this call throws NotRegistered.
        ///
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the caller.
        ///
        /// * 'netuid' (u16):
        /// 	- The u16 network identifier.
        ///
        /// * 'version' (u64):
        /// 	- The bittensor version identifier.
        ///
        /// * 'ip' (u64):
        /// 	- The endpoint ip information as a u128 encoded integer.
        ///
        /// * 'port' (u16):
        /// 	- The endpoint port information as a u16 encoded integer.
        ///
        /// * 'ip_type' (u8):
        /// 	- The endpoint ip version as a u8, 4 or 6.
        ///
        /// * 'protocol' (u8):
        /// 	- UDP:1 or TCP:0
        ///
        /// * 'placeholder1' (u8):
        /// 	- Placeholder for further extra params.
        ///
        /// * 'placeholder2' (u8):
        /// 	- Placeholder for further extra params.
        ///
        /// * 'certificate' (Vec<u8>):
        ///     - TLS certificate for inter neuron communitation.
        ///
        /// # Event:
        /// * AxonServed;
        /// 	- On successfully serving the axon info.
        ///
        /// # Raises:
        /// * 'SubNetworkDoesNotExist':
        /// 	- Attempting to set weights on a non-existent network.
        ///
        /// * 'NotRegistered':
        /// 	- Attempting to set weights from a non registered account.
        ///
        /// * 'InvalidIpType':
        /// 	- The ip type is not 4 or 6.
        ///
        /// * 'InvalidIpAddress':
        /// 	- The numerically encoded ip address does not resolve to a proper ip.
        ///
        /// * 'ServingRateLimitExceeded':
        /// 	- Attempting to set prometheus information withing the rate limit min.
        ///
        #[pallet::call_index(40)]
        #[pallet::weight((Weight::from_parts(33_890_000, 0)
		.saturating_add(T::DbWeight::get().reads(4))
		.saturating_add(T::DbWeight::get().writes(1)), DispatchClass::Normal, Pays::No))]
        pub fn serve_axon_tls(
            origin: OriginFor<T>,
            netuid: NetUid,
            version: u32,
            ip: u128,
            port: u16,
            ip_type: u8,
            protocol: u8,
            placeholder1: u8,
            placeholder2: u8,
            certificate: Vec<u8>,
        ) -> DispatchResult {
            Self::do_serve_axon(
                origin,
                netuid,
                version,
                ip,
                port,
                ip_type,
                protocol,
                placeholder1,
                placeholder2,
                Some(certificate),
            )
        }

        /// ---- Set prometheus information for the neuron.
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the calling hotkey.
        ///
        /// * 'netuid' (u16):
        /// 	- The u16 network identifier.
        ///
        /// * 'version' (u16):
        /// 	-  The bittensor version identifier.
        ///
        /// * 'ip' (u128):
        /// 	- The prometheus ip information as a u128 encoded integer.
        ///
        /// * 'port' (u16):
        /// 	- The prometheus port information as a u16 encoded integer.
        ///
        /// * 'ip_type' (u8):
        /// 	- The ip type v4 or v6.
        ///
        #[pallet::call_index(5)]
        #[pallet::weight((Weight::from_parts(31_170_000, 0)
		.saturating_add(T::DbWeight::get().reads(4))
		.saturating_add(T::DbWeight::get().writes(1)), DispatchClass::Normal, Pays::No))]
        pub fn serve_prometheus(
            origin: OriginFor<T>,
            netuid: NetUid,
            version: u32,
            ip: u128,
            port: u16,
            ip_type: u8,
        ) -> DispatchResult {
            Self::do_serve_prometheus(origin, netuid, version, ip, port, ip_type)
        }

        /// ---- Registers a new neuron to the subnetwork.
        ///
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the calling hotkey.
        ///
        /// * 'netuid' (u16):
        /// 	- The u16 network identifier.
        ///
        /// * 'block_number' ( u64 ):
        /// 	- Block hash used to prove work done.
        ///
        /// * 'nonce' ( u64 ):
        /// 	- Positive integer nonce used in POW.
        ///
        /// * 'work' ( Vec<u8> ):
        /// 	- Vector encoded bytes representing work done.
        ///
        /// * 'hotkey' ( T::AccountId ):
        /// 	- Hotkey to be registered to the network.
        ///
        /// * 'coldkey' ( T::AccountId ):
        /// 	- Associated coldkey account.
        ///
        /// # Event:
        /// * NeuronRegistered;
        /// 	- On successfully registering a uid to a neuron slot on a subnetwork.
        ///
        /// # Raises:
        /// * 'SubNetworkDoesNotExist':
        /// 	- Attempting to register to a non existent network.
        ///
        /// * 'TooManyRegistrationsThisBlock':
        /// 	- This registration exceeds the total allowed on this network this block.
        ///
        /// * 'HotKeyAlreadyRegisteredInSubNet':
        /// 	- The hotkey is already registered on this network.
        ///
        /// * 'InvalidWorkBlock':
        /// 	- The work has been performed on a stale, future, or non existent block.
        ///
        /// * 'InvalidDifficulty':
        /// 	- The work does not match the difficulty.
        ///
        /// * 'InvalidSeal':
        /// 	- The seal is incorrect.
        ///
        #[pallet::call_index(6)]
        #[pallet::weight((Weight::from_parts(216_200_000, 0)
		.saturating_add(T::DbWeight::get().reads(26))
		.saturating_add(T::DbWeight::get().writes(23)), DispatchClass::Normal, Pays::No))]
        pub fn register(
            origin: OriginFor<T>,
            netuid: NetUid,
            block_number: u64,
            nonce: u64,
            work: Vec<u8>,
            hotkey: T::AccountId,
            coldkey: T::AccountId,
        ) -> DispatchResult {
            Self::do_registration(origin, netuid, block_number, nonce, work, hotkey, coldkey)
        }

        /// Register the hotkey to root network
        #[pallet::call_index(62)]
        #[pallet::weight((Weight::from_parts(145_500_000, 0)
		.saturating_add(T::DbWeight::get().reads(23))
		.saturating_add(T::DbWeight::get().writes(20)), DispatchClass::Normal, Pays::No))]
        pub fn root_register(origin: OriginFor<T>, hotkey: T::AccountId) -> DispatchResult {
            Self::do_root_register(origin, hotkey)
        }

        /// Attempt to adjust the senate membership to include a hotkey
        #[pallet::call_index(63)]
        #[pallet::weight((Weight::from_parts(68_100_000, 0)
		.saturating_add(T::DbWeight::get().reads(7))
		.saturating_add(T::DbWeight::get().writes(4)), DispatchClass::Normal, Pays::Yes))]
        pub fn adjust_senate(origin: OriginFor<T>, hotkey: T::AccountId) -> DispatchResult {
            Self::do_adjust_senate(origin, hotkey)
        }

        /// User register a new subnetwork via burning token
        #[pallet::call_index(7)]
        #[pallet::weight((Weight::from_parts(354_400_000, 0)
		.saturating_add(T::DbWeight::get().reads(49))
		.saturating_add(T::DbWeight::get().writes(43)), DispatchClass::Normal, Pays::No))]
        pub fn burned_register(
            origin: OriginFor<T>,
            netuid: NetUid,
            hotkey: T::AccountId,
        ) -> DispatchResult {
            Self::do_burned_registration(origin, netuid, hotkey)
        }

        /// The extrinsic for user to change its hotkey in subnet or all subnets.
        #[pallet::call_index(70)]
        #[pallet::weight((Weight::from_parts(285_900_000, 0)
        .saturating_add(T::DbWeight::get().reads(47))
        .saturating_add(T::DbWeight::get().writes(37)), DispatchClass::Operational, Pays::No))]
        pub fn swap_hotkey(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            new_hotkey: T::AccountId,
            netuid: Option<NetUid>,
        ) -> DispatchResultWithPostInfo {
            Self::do_swap_hotkey(origin, &hotkey, &new_hotkey, netuid)
        }

        /// The extrinsic for user to change the coldkey associated with their account.
        ///
        /// # Arguments
        ///
        /// * `origin` - The origin of the call, must be signed by the old coldkey.
        /// * `old_coldkey` - The current coldkey associated with the account.
        /// * `new_coldkey` - The new coldkey to be associated with the account.
        ///
        /// # Returns
        ///
        /// Returns a `DispatchResultWithPostInfo` indicating success or failure of the operation.
        ///
        /// # Weight
        ///
        /// Weight is calculated based on the number of database reads and writes.
        #[pallet::call_index(71)]
        #[pallet::weight((Weight::from_parts(208600000, 0)
        .saturating_add(T::DbWeight::get().reads(14))
        .saturating_add(T::DbWeight::get().writes(9)), DispatchClass::Operational, Pays::No))]
        pub fn swap_coldkey(
            origin: OriginFor<T>,
            old_coldkey: T::AccountId,
            new_coldkey: T::AccountId,
            swap_cost: u64,
        ) -> DispatchResultWithPostInfo {
            // Ensure it's called with root privileges (scheduler has root privileges)
            ensure_root(origin)?;
            log::debug!("swap_coldkey: {:?} -> {:?}", old_coldkey, new_coldkey);

            Self::do_swap_coldkey(&old_coldkey, &new_coldkey, swap_cost)
        }

        /// Sets the childkey take for a given hotkey.
        ///
        /// This function allows a coldkey to set the childkey take for a given hotkey.
        /// The childkey take determines the proportion of stake that the hotkey keeps for itself
        /// when distributing stake to its children.
        ///
        /// # Arguments:
        /// * `origin` (<T as frame_system::Config>::RuntimeOrigin):
        ///     - The signature of the calling coldkey. Setting childkey take can only be done by the coldkey.
        ///
        /// * `hotkey` (T::AccountId):
        ///     - The hotkey for which the childkey take will be set.
        ///
        /// * `take` (u16):
        ///     - The new childkey take value. This is a percentage represented as a value between 0 and 10000,
        ///       where 10000 represents 100%.
        ///
        /// # Events:
        /// * `ChildkeyTakeSet`:
        ///     - On successfully setting the childkey take for a hotkey.
        ///
        /// # Errors:
        /// * `NonAssociatedColdKey`:
        ///     - The coldkey does not own the hotkey.
        /// * `InvalidChildkeyTake`:
        ///     - The provided take value is invalid (greater than the maximum allowed take).
        /// * `TxChildkeyTakeRateLimitExceeded`:
        ///     - The rate limit for changing childkey take has been exceeded.
        ///
        #[pallet::call_index(75)]
        #[pallet::weight((
            Weight::from_parts(46_330_000, 0)
            .saturating_add(T::DbWeight::get().reads(5))
            .saturating_add(T::DbWeight::get().writes(2)),
    DispatchClass::Normal,
    Pays::Yes
))]
        pub fn set_childkey_take(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            netuid: NetUid,
            take: u16,
        ) -> DispatchResult {
            let coldkey = ensure_signed(origin)?;

            // Call the utility function to set the childkey take
            Self::do_set_childkey_take(coldkey, hotkey, netuid, take)
        }

        // ---- SUDO ONLY FUNCTIONS ------------------------------------------------------------

        /// Sets the transaction rate limit for changing childkey take.
        ///
        /// This function can only be called by the root origin.
        ///
        /// # Arguments:
        /// * `origin` - The origin of the call, must be root.
        /// * `tx_rate_limit` - The new rate limit in blocks.
        ///
        /// # Errors:
        /// * `BadOrigin` - If the origin is not root.
        ///
        #[pallet::call_index(69)]
        #[pallet::weight((
            Weight::from_parts(5_760_000, 0)
            .saturating_add(T::DbWeight::get().reads(0))
            .saturating_add(T::DbWeight::get().writes(1)),
            DispatchClass::Operational,
            Pays::No
        ))]
        pub fn sudo_set_tx_childkey_take_rate_limit(
            origin: OriginFor<T>,
            tx_rate_limit: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::set_tx_childkey_take_rate_limit(tx_rate_limit);
            Ok(())
        }

        /// Sets the minimum allowed childkey take.
        ///
        /// This function can only be called by the root origin.
        ///
        /// # Arguments:
        /// * `origin` - The origin of the call, must be root.
        /// * `take` - The new minimum childkey take value.
        ///
        /// # Errors:
        /// * `BadOrigin` - If the origin is not root.
        ///
        #[pallet::call_index(76)]
        #[pallet::weight((
            Weight::from_parts(6_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1)),
            DispatchClass::Operational,
            Pays::No
        ))]
        pub fn sudo_set_min_childkey_take(origin: OriginFor<T>, take: u16) -> DispatchResult {
            ensure_root(origin)?;
            Self::set_min_childkey_take(take);
            Ok(())
        }

        /// Sets the maximum allowed childkey take.
        ///
        /// This function can only be called by the root origin.
        ///
        /// # Arguments:
        /// * `origin` - The origin of the call, must be root.
        /// * `take` - The new maximum childkey take value.
        ///
        /// # Errors:
        /// * `BadOrigin` - If the origin is not root.
        ///
        #[pallet::call_index(77)]
        #[pallet::weight((
            Weight::from_parts(6_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1)),
            DispatchClass::Operational,
            Pays::No
        ))]
        pub fn sudo_set_max_childkey_take(origin: OriginFor<T>, take: u16) -> DispatchResult {
            ensure_root(origin)?;
            Self::set_max_childkey_take(take);
            Ok(())
        }
        // ==================================
        // ==== Parameter Sudo calls ========
        // ==================================
        // Each function sets the corresponding hyper paramter on the specified network
        // Args:
        // 	* 'origin': (<T as frame_system::Config>Origin):
        // 		- The caller, must be sudo.
        //
        // 	* `netuid` (u16):
        // 		- The network identifier.
        //
        // 	* `hyperparameter value` (u16):
        // 		- The value of the hyper parameter.
        //

        /// Authenticates a council proposal and dispatches a function call with `Root` origin.
        ///
        /// The dispatch origin for this call must be a council majority.
        ///
        /// ## Complexity
        /// - O(1).
        #[pallet::call_index(51)]
        #[pallet::weight((Weight::from_parts(111_100_000, 0), DispatchClass::Operational, Pays::No))]
        pub fn sudo(
            origin: OriginFor<T>,
            call: Box<T::SudoRuntimeCall>,
        ) -> DispatchResultWithPostInfo {
            // This is a public call, so we ensure that the origin is a council majority.
            T::CouncilOrigin::ensure_origin(origin)?;

            let result = call.dispatch_bypass_filter(frame_system::RawOrigin::Root.into());
            let error = result.map(|_| ()).map_err(|e| e.error);
            Self::deposit_event(Event::Sudid(error));

            return result;
        }

        /// Authenticates a council proposal and dispatches a function call with `Root` origin.
        /// This function does not check the weight of the call, and instead allows the
        /// user to specify the weight of the call.
        ///
        /// The dispatch origin for this call must be a council majority.
        ///
        /// ## Complexity
        /// - O(1).
        #[allow(deprecated)]
        #[pallet::call_index(52)]
        #[pallet::weight((*weight, call.get_dispatch_info().class, Pays::No))]
        pub fn sudo_unchecked_weight(
            origin: OriginFor<T>,
            call: Box<T::SudoRuntimeCall>,
            weight: Weight,
        ) -> DispatchResultWithPostInfo {
            // We dont need to check the weight witness, suppress warning.
            // See https://github.com/paritytech/polkadot-sdk/pull/1818.
            let _ = weight;

            // This is a public call, so we ensure that the origin is a council majority.
            T::CouncilOrigin::ensure_origin(origin)?;

            let result = call.dispatch_bypass_filter(frame_system::RawOrigin::Root.into());
            let error = result.map(|_| ()).map_err(|e| e.error);
            Self::deposit_event(Event::Sudid(error));

            return result;
        }

        /// User vote on a proposal
        #[pallet::call_index(55)]
        #[pallet::weight((Weight::from_parts(111_100_000, 0)
		.saturating_add(T::DbWeight::get().reads(0))
		.saturating_add(T::DbWeight::get().writes(0)), DispatchClass::Operational))]
        pub fn vote(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            proposal: T::Hash,
            #[pallet::compact] index: u32,
            approve: bool,
        ) -> DispatchResultWithPostInfo {
            Self::do_vote_root(origin, &hotkey, proposal, index, approve)
        }

        /// User register a new subnetwork
        #[pallet::call_index(59)]
        #[pallet::weight((Weight::from_parts(260_500_000, 0)
		.saturating_add(T::DbWeight::get().reads(36))
		.saturating_add(T::DbWeight::get().writes(52)), DispatchClass::Operational, Pays::No))]
        pub fn register_network(origin: OriginFor<T>, hotkey: T::AccountId) -> DispatchResult {
            Self::do_register_network(origin, &hotkey, 1, None)
        }

        /// Facility extrinsic for user to get taken from faucet
        /// It is only available when pow-faucet feature enabled
        /// Just deployed in testnet and devnet for testing purpose
        #[pallet::call_index(60)]
        #[pallet::weight((Weight::from_parts(91_000_000, 0)
        .saturating_add(T::DbWeight::get().reads(27))
		.saturating_add(T::DbWeight::get().writes(22)), DispatchClass::Normal, Pays::No))]
        pub fn faucet(
            origin: OriginFor<T>,
            block_number: u64,
            nonce: u64,
            work: Vec<u8>,
        ) -> DispatchResult {
            if cfg!(feature = "pow-faucet") {
                return Self::do_faucet(origin, block_number, nonce, work);
            }

            Err(Error::<T>::FaucetDisabled.into())
        }

        /// Remove a user's subnetwork
        /// The caller must be the owner of the network
        #[pallet::call_index(61)]
        #[pallet::weight((Weight::from_parts(119_000_000, 0)
		.saturating_add(T::DbWeight::get().reads(6))
		.saturating_add(T::DbWeight::get().writes(31)), DispatchClass::Operational, Pays::No))]
        pub fn dissolve_network(
            origin: OriginFor<T>,
            coldkey: T::AccountId,
            netuid: NetUid,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::user_remove_network(coldkey, netuid)
        }

        /// Set a single child for a given hotkey on a specified network.
        ///
        /// This function allows a coldkey to set a single child for a given hotkey on a specified network.
        /// The proportion of the hotkey's stake to be allocated to the child is also specified.
        ///
        /// # Arguments:
        /// * `origin` (<T as frame_system::Config>::RuntimeOrigin):
        ///     - The signature of the calling coldkey. Setting a hotkey child can only be done by the coldkey.
        ///
        /// * `hotkey` (T::AccountId):
        ///     - The hotkey which will be assigned the child.
        ///
        /// * `child` (T::AccountId):
        ///     - The child which will be assigned to the hotkey.
        ///
        /// * `netuid` (u16):
        ///     - The u16 network identifier where the childkey will exist.
        ///
        /// * `proportion` (u64):
        ///     - Proportion of the hotkey's stake to be given to the child, the value must be u64 normalized.
        ///
        /// # Events:
        /// * `ChildAddedSingular`:
        ///     - On successfully registering a child to a hotkey.
        ///
        /// # Errors:
        /// * `SubNetworkDoesNotExist`:
        ///     - Attempting to register to a non-existent network.
        /// * `RegistrationNotPermittedOnRootSubnet`:
        ///     - Attempting to register a child on the root network.
        /// * `NonAssociatedColdKey`:
        ///     - The coldkey does not own the hotkey or the child is the same as the hotkey.
        /// * `HotKeyAccountNotExists`:
        ///     - The hotkey account does not exist.
        ///
        /// # Detailed Explanation of Checks:
        /// 1. **Signature Verification**: Ensures that the caller has signed the transaction, verifying the coldkey.
        /// 2. **Root Network Check**: Ensures that the delegation is not on the root network, as child hotkeys are not valid on the root.
        /// 3. **Network Existence Check**: Ensures that the specified network exists.
        /// 4. **Ownership Verification**: Ensures that the coldkey owns the hotkey.
        /// 5. **Hotkey Account Existence Check**: Ensures that the hotkey account already exists.
        /// 6. **Child-Hotkey Distinction**: Ensures that the child is not the same as the hotkey.
        /// 7. **Old Children Cleanup**: Removes the hotkey from the parent list of its old children.
        /// 8. **New Children Assignment**: Assigns the new child to the hotkey and updates the parent list for the new child.
        // TODO: Benchmark this call
        #[pallet::call_index(67)]
        #[pallet::weight((Weight::from_parts(119_000_000, 0)
		.saturating_add(T::DbWeight::get().reads(6))
		.saturating_add(T::DbWeight::get().writes(31)), DispatchClass::Operational, Pays::Yes))]
        pub fn set_children(
            origin: T::RuntimeOrigin,
            hotkey: T::AccountId,
            netuid: NetUid,
            children: Vec<(u64, T::AccountId)>,
        ) -> DispatchResultWithPostInfo {
            Self::do_schedule_children(origin, hotkey, netuid, children)?;
            Ok(().into())
        }

        /// Schedules a coldkey swap operation to be executed at a future block.
        ///
        /// This function allows a user to schedule the swapping of their coldkey to a new one
        /// at a specified future block. The swap is not executed immediately but is scheduled
        /// to occur at the specified block number.
        ///
        /// # Arguments
        ///
        /// * `origin` - The origin of the call, which should be signed by the current coldkey owner.
        /// * `new_coldkey` - The account ID of the new coldkey that will replace the current one.
        /// * `when` - The block number at which the coldkey swap should be executed.
        ///
        /// # Returns
        ///
        /// Returns a `DispatchResultWithPostInfo` indicating whether the scheduling was successful.
        ///
        /// # Errors
        ///
        /// This function may return an error if:
        /// * The origin is not signed.
        /// * The scheduling fails due to conflicts or system constraints.
        ///
        /// # Notes
        ///
        /// - The actual swap is not performed by this function. It merely schedules the swap operation.
        /// - The weight of this call is set to a fixed value and may need adjustment based on benchmarking.
        ///
        /// # TODO
        ///
        /// - Implement proper weight calculation based on the complexity of the operation.
        /// - Consider adding checks to prevent scheduling too far into the future.
        /// TODO: Benchmark this call
        #[pallet::call_index(73)]
        #[pallet::weight((Weight::from_parts(44_520_000, 0)
		.saturating_add(T::DbWeight::get().reads(4))
		.saturating_add(T::DbWeight::get().writes(2)), DispatchClass::Operational, Pays::Yes))]
        pub fn schedule_swap_coldkey(
            origin: OriginFor<T>,
            new_coldkey: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let current_block = <frame_system::Pallet<T>>::block_number();

            // If the coldkey has a scheduled swap, check if we can reschedule it
            if ColdkeySwapScheduled::<T>::contains_key(&who) {
                let (scheduled_block, _scheduled_coldkey) = ColdkeySwapScheduled::<T>::get(&who);
                let reschedule_duration = ColdkeySwapRescheduleDuration::<T>::get();
                let redo_when = scheduled_block.saturating_add(reschedule_duration);
                ensure!(redo_when <= current_block, Error::<T>::SwapAlreadyScheduled);
            }

            // Calculate the swap cost and ensure sufficient balance
            let swap_cost = Self::get_key_swap_cost();
            ensure!(
                Self::can_remove_balance_from_coldkey_account(&who, swap_cost),
                Error::<T>::NotEnoughBalanceToPaySwapColdKey
            );

            let current_block: BlockNumberFor<T> = <frame_system::Pallet<T>>::block_number();
            let duration: BlockNumberFor<T> = ColdkeySwapScheduleDuration::<T>::get();
            let when: BlockNumberFor<T> = current_block.saturating_add(duration);

            let call = Call::<T>::swap_coldkey {
                old_coldkey: who.clone(),
                new_coldkey: new_coldkey.clone(),
                swap_cost,
            };

            let bound_call = <T as Config>::Preimages::bound(LocalCallOf::<T>::from(call.clone()))
                .map_err(|_| Error::<T>::FailedToSchedule)?;

            T::Scheduler::schedule(
                DispatchTime::At(when),
                None,
                63,
                frame_system::RawOrigin::Root.into(),
                bound_call,
            )
            .map_err(|_| Error::<T>::FailedToSchedule)?;

            ColdkeySwapScheduled::<T>::insert(&who, (when, new_coldkey.clone()));
            // Emit the SwapScheduled event
            Self::deposit_event(Event::ColdkeySwapScheduled {
                old_coldkey: who.clone(),
                new_coldkey: new_coldkey.clone(),
                execution_block: when,
                swap_cost,
            });

            Ok(().into())
        }

        /// Schedule the dissolution of a network at a specified block number.
        ///
        /// # Arguments
        ///
        /// * `origin` - The origin of the call, must be signed by the sender.
        /// * `netuid` - The u16 network identifier to be dissolved.
        ///
        /// # Returns
        ///
        /// Returns a `DispatchResultWithPostInfo` indicating success or failure of the operation.
        ///
        /// # Weight
        ///
        /// Weight is calculated based on the number of database reads and writes.

        #[pallet::call_index(74)]
        #[pallet::weight((Weight::from_parts(119_000_000, 0)
		.saturating_add(T::DbWeight::get().reads(6))
		.saturating_add(T::DbWeight::get().writes(31)), DispatchClass::Operational, Pays::Yes))]
        pub fn schedule_dissolve_network(
            _origin: OriginFor<T>,
            _netuid: NetUid,
        ) -> DispatchResultWithPostInfo {
            Err(Error::<T>::CallDisabled.into())

            // let who = ensure_signed(origin)?;

            // let current_block: BlockNumberFor<T> = <frame_system::Pallet<T>>::block_number();
            // let duration: BlockNumberFor<T> = DissolveNetworkScheduleDuration::<T>::get();
            // let when: BlockNumberFor<T> = current_block.saturating_add(duration);

            // let call = Call::<T>::dissolve_network {
            //     coldkey: who.clone(),
            //     netuid,
            // };

            // let bound_call = T::Preimages::bound(LocalCallOf::<T>::from(call.clone()))
            //     .map_err(|_| Error::<T>::FailedToSchedule)?;

            // T::Scheduler::schedule(
            //     DispatchTime::At(when),
            //     None,
            //     63,
            //     frame_system::RawOrigin::Root.into(),
            //     bound_call,
            // )
            // .map_err(|_| Error::<T>::FailedToSchedule)?;

            // // Emit the SwapScheduled event
            // Self::deposit_event(Event::DissolveNetworkScheduled {
            //     account: who.clone(),
            //     netuid,
            //     execution_block: when,
            // });

            // Ok(().into())
        }

        /// ---- Set prometheus information for the neuron.
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the calling hotkey.
        ///
        /// * 'netuid' (u16):
        /// 	- The u16 network identifier.
        ///
        /// * 'version' (u16):
        /// 	-  The bittensor version identifier.
        ///
        /// * 'ip' (u128):
        /// 	- The prometheus ip information as a u128 encoded integer.
        ///
        /// * 'port' (u16):
        /// 	- The prometheus port information as a u16 encoded integer.
        ///
        /// * 'ip_type' (u8):
        /// 	- The ip type v4 or v6.
        ///
        #[pallet::call_index(68)]
        #[pallet::weight((Weight::from_parts(31_780_000, 0)
		.saturating_add(T::DbWeight::get().reads(3))
		.saturating_add(T::DbWeight::get().writes(1)), DispatchClass::Normal, Pays::Yes))]
        pub fn set_identity(
            origin: OriginFor<T>,
            name: Vec<u8>,
            url: Vec<u8>,
            github_repo: Vec<u8>,
            image: Vec<u8>,
            discord: Vec<u8>,
            description: Vec<u8>,
            additional: Vec<u8>,
        ) -> DispatchResult {
            Self::do_set_identity(
                origin,
                name,
                url,
                github_repo,
                image,
                discord,
                description,
                additional,
            )
        }

        /// ---- Set the identity information for a subnet.
        /// # Args:
        /// * `origin` - (<T as frame_system::Config>::Origin):
        ///     - The signature of the calling coldkey, which must be the owner of the subnet.
        ///
        /// * `netuid` (u16):
        ///     - The unique network identifier of the subnet.
        ///
        /// * `subnet_name` (Vec<u8>):
        ///     - The name of the subnet.
        ///
        /// * `github_repo` (Vec<u8>):
        ///     - The GitHub repository associated with the subnet identity.
        ///
        /// * `subnet_contact` (Vec<u8>):
        ///     - The contact information for the subnet.
        #[pallet::call_index(78)]
        #[pallet::weight((Weight::from_parts(23_080_000, 0)
		.saturating_add(T::DbWeight::get().reads(1))
		.saturating_add(T::DbWeight::get().writes(1)), DispatchClass::Normal, Pays::Yes))]
        pub fn set_subnet_identity(
            origin: OriginFor<T>,
            netuid: NetUid,
            subnet_name: Vec<u8>,
            github_repo: Vec<u8>,
            subnet_contact: Vec<u8>,
            subnet_url: Vec<u8>,
            discord: Vec<u8>,
            description: Vec<u8>,
            logo_url: Vec<u8>,
            additional: Vec<u8>,
        ) -> DispatchResult {
            Self::do_set_subnet_identity(
                origin,
                netuid,
                subnet_name,
                github_repo,
                subnet_contact,
                subnet_url,
                discord,
                description,
                logo_url,
                additional,
            )
        }

        /// User register a new subnetwork
        #[pallet::call_index(79)]
        #[pallet::weight((Weight::from_parts(239_700_000, 0)
                .saturating_add(T::DbWeight::get().reads(35))
                .saturating_add(T::DbWeight::get().writes(51)), DispatchClass::Operational, Pays::No))]
        pub fn register_network_with_identity(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            identity: Option<SubnetIdentityOfV3>,
        ) -> DispatchResult {
            Self::do_register_network(origin, &hotkey, 1, identity)
        }

        /// ---- The implementation for the extrinsic unstake_all: Removes all stake from a hotkey account across all subnets and adds it onto a coldkey.
        ///
        /// # Args:
        /// * `origin` - (<T as frame_system::Config>::Origin):
        ///     - The signature of the caller's coldkey.
        ///
        /// * `hotkey` (T::AccountId):
        ///     - The associated hotkey account.
        ///
        /// # Event:
        /// * StakeRemoved;
        ///     - On the successfully removing stake from the hotkey account.
        ///
        /// # Raises:
        /// * `NotRegistered`:
        ///     - Thrown if the account we are attempting to unstake from is non existent.
        ///
        /// * `NonAssociatedColdKey`:
        ///     - Thrown if the coldkey does not own the hotkey we are unstaking from.
        ///
        /// * `NotEnoughStakeToWithdraw`:
        ///     - Thrown if there is not enough stake on the hotkey to withdraw this amount.
        ///
        /// * `TxRateLimitExceeded`:
        ///     - Thrown if key has hit transaction rate limit
        #[pallet::call_index(83)]
        #[pallet::weight((Weight::from_parts(30_190_000, 0)
        .saturating_add(T::DbWeight::get().reads(6))
        .saturating_add(T::DbWeight::get().writes(0)), DispatchClass::Operational, Pays::Yes))]
        pub fn unstake_all(origin: OriginFor<T>, hotkey: T::AccountId) -> DispatchResult {
            Self::do_unstake_all(origin, hotkey)
        }

        /// ---- The implementation for the extrinsic unstake_all: Removes all stake from a hotkey account across all subnets and adds it onto a coldkey.
        ///
        /// # Args:
        /// * `origin` - (<T as frame_system::Config>::Origin):
        ///     - The signature of the caller's coldkey.
        ///
        /// * `hotkey` (T::AccountId):
        ///     - The associated hotkey account.
        ///
        /// # Event:
        /// * StakeRemoved;
        ///     - On the successfully removing stake from the hotkey account.
        ///
        /// # Raises:
        /// * `NotRegistered`:
        ///     - Thrown if the account we are attempting to unstake from is non existent.
        ///
        /// * `NonAssociatedColdKey`:
        ///     - Thrown if the coldkey does not own the hotkey we are unstaking from.
        ///
        /// * `NotEnoughStakeToWithdraw`:
        ///     - Thrown if there is not enough stake on the hotkey to withdraw this amount.
        ///
        /// * `TxRateLimitExceeded`:
        ///     - Thrown if key has hit transaction rate limit
        #[pallet::call_index(84)]
        #[pallet::weight((Weight::from_parts(369_500_000, 0)
        .saturating_add(T::DbWeight::get().reads(33))
        .saturating_add(T::DbWeight::get().writes(16)), DispatchClass::Operational, Pays::Yes))]
        pub fn unstake_all_alpha(origin: OriginFor<T>, hotkey: T::AccountId) -> DispatchResult {
            Self::do_unstake_all_alpha(origin, hotkey)
        }

        /// ---- The implementation for the extrinsic move_stake: Moves specified amount of stake from a hotkey to another across subnets.
        ///
        /// # Args:
        /// * `origin` - (<T as frame_system::Config>::Origin):
        ///     - The signature of the caller's coldkey.
        ///
        /// * `origin_hotkey` (T::AccountId):
        ///     - The hotkey account to move stake from.
        ///
        /// * `destination_hotkey` (T::AccountId):
        ///     - The hotkey account to move stake to.
        ///
        /// * `origin_netuid` (T::AccountId):
        ///     - The subnet ID to move stake from.
        ///
        /// * `destination_netuid` (T::AccountId):
        ///     - The subnet ID to move stake to.
        ///
        /// * `alpha_amount` (T::AccountId):
        ///     - The alpha stake amount to move.
        ///
        #[pallet::call_index(85)]
        #[pallet::weight((Weight::from_parts(157_100_000, 0)
        .saturating_add(T::DbWeight::get().reads(15_u64))
        .saturating_add(T::DbWeight::get().writes(7_u64)), DispatchClass::Operational, Pays::Yes))]
        pub fn move_stake(
            origin: T::RuntimeOrigin,
            origin_hotkey: T::AccountId,
            destination_hotkey: T::AccountId,
            origin_netuid: NetUid,
            destination_netuid: NetUid,
            alpha_amount: AlphaCurrency,
        ) -> DispatchResult {
            Self::do_move_stake(
                origin,
                origin_hotkey,
                destination_hotkey,
                origin_netuid,
                destination_netuid,
                alpha_amount,
            )
        }

        /// Transfers a specified amount of stake from one coldkey to another, optionally across subnets,
        /// while keeping the same hotkey.
        ///
        /// # Arguments
        /// * `origin` - The origin of the transaction, which must be signed by the `origin_coldkey`.
        /// * `destination_coldkey` - The coldkey to which the stake is transferred.
        /// * `hotkey` - The hotkey associated with the stake.
        /// * `origin_netuid` - The network/subnet ID to move stake from.
        /// * `destination_netuid` - The network/subnet ID to move stake to (for cross-subnet transfer).
        /// * `alpha_amount` - The amount of stake to transfer.
        ///
        /// # Errors
        /// Returns an error if:
        /// * The origin is not signed by the correct coldkey.
        /// * Either subnet does not exist.
        /// * The hotkey does not exist.
        /// * There is insufficient stake on `(origin_coldkey, hotkey, origin_netuid)`.
        /// * The transfer amount is below the minimum stake requirement.
        ///
        /// # Events
        /// May emit a `StakeTransferred` event on success.
        #[pallet::call_index(86)]
        #[pallet::weight((Weight::from_parts(154_800_000, 0)
        .saturating_add(T::DbWeight::get().reads(13_u64))
        .saturating_add(T::DbWeight::get().writes(6_u64)), DispatchClass::Operational, Pays::Yes))]
        pub fn transfer_stake(
            origin: T::RuntimeOrigin,
            destination_coldkey: T::AccountId,
            hotkey: T::AccountId,
            origin_netuid: NetUid,
            destination_netuid: NetUid,
            alpha_amount: AlphaCurrency,
        ) -> DispatchResult {
            Self::do_transfer_stake(
                origin,
                destination_coldkey,
                hotkey,
                origin_netuid,
                destination_netuid,
                alpha_amount,
            )
        }

        /// Swaps a specified amount of stake from one subnet to another, while keeping the same coldkey and hotkey.
        ///
        /// # Arguments
        /// * `origin` - The origin of the transaction, which must be signed by the coldkey that owns the `hotkey`.
        /// * `hotkey` - The hotkey whose stake is being swapped.
        /// * `origin_netuid` - The network/subnet ID from which stake is removed.
        /// * `destination_netuid` - The network/subnet ID to which stake is added.
        /// * `alpha_amount` - The amount of stake to swap.
        ///
        /// # Errors
        /// Returns an error if:
        /// * The transaction is not signed by the correct coldkey (i.e., `coldkey_owns_hotkey` fails).
        /// * Either `origin_netuid` or `destination_netuid` does not exist.
        /// * The hotkey does not exist.
        /// * There is insufficient stake on `(coldkey, hotkey, origin_netuid)`.
        /// * The swap amount is below the minimum stake requirement.
        ///
        /// # Events
        /// May emit a `StakeSwapped` event on success.
        #[pallet::call_index(87)]
        #[pallet::weight((
            Weight::from_parts(351_300_000, 0)
            .saturating_add(T::DbWeight::get().reads(32))
            .saturating_add(T::DbWeight::get().writes(17)),
            DispatchClass::Operational,
            Pays::Yes
        ))]
        pub fn swap_stake(
            origin: T::RuntimeOrigin,
            hotkey: T::AccountId,
            origin_netuid: NetUid,
            destination_netuid: NetUid,
            alpha_amount: AlphaCurrency,
        ) -> DispatchResult {
            Self::do_swap_stake(
                origin,
                hotkey,
                origin_netuid,
                destination_netuid,
                alpha_amount,
            )
        }

        /// --- Adds stake to a hotkey on a subnet with a price limit.
        /// This extrinsic allows to specify the limit price for alpha token
        /// at which or better (lower) the staking should execute.
        ///
        /// In case if slippage occurs and the price shall move beyond the limit
        /// price, the staking order may execute only partially or not execute
        /// at all.
        ///
        /// # Args:
        ///  * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the caller's coldkey.
        ///
        ///  * 'hotkey' (T::AccountId):
        /// 	- The associated hotkey account.
        ///
        /// * 'netuid' (u16):
        ///     - Subnetwork UID
        ///
        ///  * 'amount_staked' (u64):
        /// 	- The amount of stake to be added to the hotkey staking account.
        ///
        ///  * 'limit_price' (u64):
        /// 	- The limit price expressed in units of RAO per one Alpha.
        ///
        ///  * 'allow_partial' (bool):
        /// 	- Allows partial execution of the amount. If set to false, this becomes
        ///       fill or kill type or order.
        ///
        /// # Event:
        ///  * StakeAdded;
        /// 	- On the successfully adding stake to a global account.
        ///
        /// # Raises:
        ///  * 'NotEnoughBalanceToStake':
        /// 	- Not enough balance on the coldkey to add onto the global account.
        ///
        ///  * 'NonAssociatedColdKey':
        /// 	- The calling coldkey is not associated with this hotkey.
        ///
        ///  * 'BalanceWithdrawalError':
        ///  	- Errors stemming from transaction pallet.
        ///
        #[pallet::call_index(88)]
        #[pallet::weight((Weight::from_parts(402_800_000, 0)
		.saturating_add(T::DbWeight::get().reads(26))
		.saturating_add(T::DbWeight::get().writes(15)), DispatchClass::Normal, Pays::Yes))]
        pub fn add_stake_limit(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            netuid: NetUid,
            amount_staked: u64,
            limit_price: u64,
            allow_partial: bool,
        ) -> DispatchResult {
            Self::do_add_stake_limit(
                origin,
                hotkey,
                netuid,
                amount_staked,
                limit_price,
                allow_partial,
            )
        }

        /// --- Removes stake from a hotkey on a subnet with a price limit.
        /// This extrinsic allows to specify the limit price for alpha token
        /// at which or better (higher) the staking should execute.
        ///
        /// In case if slippage occurs and the price shall move beyond the limit
        /// price, the staking order may execute only partially or not execute
        /// at all.
        ///
        /// # Args:
        /// * 'origin': (<T as frame_system::Config>Origin):
        /// 	- The signature of the caller's coldkey.
        ///
        /// * 'hotkey' (T::AccountId):
        /// 	- The associated hotkey account.
        ///
        /// * 'netuid' (u16):
        ///     - Subnetwork UID
        ///
        /// * 'amount_unstaked' (u64):
        /// 	- The amount of stake to be added to the hotkey staking account.
        ///
        ///  * 'limit_price' (u64):
        ///     - The limit price expressed in units of RAO per one Alpha.
        ///
        ///  * 'allow_partial' (bool):
        ///     - Allows partial execution of the amount. If set to false, this becomes
        ///       fill or kill type or order.
        ///
        /// # Event:
        /// * StakeRemoved;
        /// 	- On the successfully removing stake from the hotkey account.
        ///
        /// # Raises:
        /// * 'NotRegistered':
        /// 	- Thrown if the account we are attempting to unstake from is non existent.
        ///
        /// * 'NonAssociatedColdKey':
        /// 	- Thrown if the coldkey does not own the hotkey we are unstaking from.
        ///
        /// * 'NotEnoughStakeToWithdraw':
        /// 	- Thrown if there is not enough stake on the hotkey to withdwraw this amount.
        ///
        #[pallet::call_index(89)]
        #[pallet::weight((Weight::from_parts(403_800_000, 0)
		.saturating_add(T::DbWeight::get().reads(30))
		.saturating_add(T::DbWeight::get().writes(14)), DispatchClass::Normal, Pays::Yes))]
        pub fn remove_stake_limit(
            origin: OriginFor<T>,
            hotkey: T::AccountId,
            netuid: NetUid,
            amount_unstaked: AlphaCurrency,
            limit_price: u64,
            allow_partial: bool,
        ) -> DispatchResult {
            Self::do_remove_stake_limit(
                origin,
                hotkey,
                netuid,
                amount_unstaked,
                limit_price,
                allow_partial,
            )
        }

        /// Swaps a specified amount of stake from one subnet to another, while keeping the same coldkey and hotkey.
        ///
        /// # Arguments
        /// * `origin` - The origin of the transaction, which must be signed by the coldkey that owns the `hotkey`.
        /// * `hotkey` - The hotkey whose stake is being swapped.
        /// * `origin_netuid` - The network/subnet ID from which stake is removed.
        /// * `destination_netuid` - The network/subnet ID to which stake is added.
        /// * `alpha_amount` - The amount of stake to swap.
        /// * `limit_price` - The limit price expressed in units of RAO per one Alpha.
        /// * `allow_partial` - Allows partial execution of the amount. If set to false, this becomes fill or kill type or order.
        ///
        /// # Errors
        /// Returns an error if:
        /// * The transaction is not signed by the correct coldkey (i.e., `coldkey_owns_hotkey` fails).
        /// * Either `origin_netuid` or `destination_netuid` does not exist.
        /// * The hotkey does not exist.
        /// * There is insufficient stake on `(coldkey, hotkey, origin_netuid)`.
        /// * The swap amount is below the minimum stake requirement.
        ///
        /// # Events
        /// May emit a `StakeSwapped` event on success.
        #[pallet::call_index(90)]
        #[pallet::weight((
            Weight::from_parts(426_500_000, 0)
            .saturating_add(T::DbWeight::get().reads(32))
            .saturating_add(T::DbWeight::get().writes(17)),
            DispatchClass::Operational,
            Pays::Yes
        ))]
        pub fn swap_stake_limit(
            origin: T::RuntimeOrigin,
            hotkey: T::AccountId,
            origin_netuid: NetUid,
            destination_netuid: NetUid,
            alpha_amount: AlphaCurrency,
            limit_price: u64,
            allow_partial: bool,
        ) -> DispatchResult {
            Self::do_swap_stake_limit(
                origin,
                hotkey,
                origin_netuid,
                destination_netuid,
                alpha_amount,
                limit_price,
                allow_partial,
            )
        }

        /// Attempts to associate a hotkey with a coldkey.
        ///
        /// # Arguments
        /// * `origin` - The origin of the transaction, which must be signed by the coldkey that owns the `hotkey`.
        /// * `hotkey` - The hotkey to associate with the coldkey.
        ///
        /// # Note
        /// Will charge based on the weight even if the hotkey is already associated with a coldkey.
        #[pallet::call_index(91)]
        #[pallet::weight((
            Weight::from_parts(32_560_000, 0).saturating_add(T::DbWeight::get().reads_writes(3, 3)),
            DispatchClass::Operational,
            Pays::Yes
        ))]
        pub fn try_associate_hotkey(
            origin: T::RuntimeOrigin,
            hotkey: T::AccountId,
        ) -> DispatchResult {
            let coldkey = ensure_signed(origin)?;

            let _ = Self::do_try_associate_hotkey(&coldkey, &hotkey);

            Ok(())
        }

        /// Initiates a call on a subnet.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be signed by the subnet owner.
        /// * `netuid` - The unique identifier of the subnet on which the call is being initiated.
        ///
        /// # Events
        /// Emits a `FirstEmissionBlockNumberSet` event on success.
        #[pallet::call_index(92)]
        #[pallet::weight((
            Weight::from_parts(35_770_000, 0).saturating_add(T::DbWeight::get().reads_writes(4, 2)),
            DispatchClass::Operational,
            Pays::Yes
        ))]
        pub fn start_call(origin: T::RuntimeOrigin, netuid: NetUid) -> DispatchResult {
            Self::do_start_call(origin, netuid)?;
            Ok(())
        }

        /// Attempts to associate a hotkey with an EVM key.
        ///
        /// The signature will be checked to see if the recovered public key matches the `evm_key` provided.
        ///
        /// The EVM key is expected to sign the message according to this formula to produce the signature:
        /// ```text
        /// keccak_256(hotkey ++ keccak_256(block_number))
        /// ```
        ///
        /// # Arguments
        /// * `origin` - The origin of the transaction, which must be signed by the coldkey that owns the `hotkey`.
        /// * `netuid` - The netuid that the `hotkey` belongs to.
        /// * `hotkey` - The hotkey associated with the `origin`.
        /// * `evm_key` - The EVM key to associate with the `hotkey`.
        /// * `block_number` - The block number used in the `signature`.
        /// * `signature` - A signed message by the `evm_key` containing the `hotkey` and the hashed `block_number`.
        ///
        /// # Errors
        /// Returns an error if:
        /// * The transaction is not signed.
        /// * The hotkey is not owned by the origin coldkey.
        /// * The hotkey does not belong to the subnet identified by the netuid.
        /// * The EVM key cannot be recovered from the signature.
        /// * The EVM key recovered from the signature does not match the given EVM key.
        ///
        /// # Events
        /// May emit a `EvmKeyAssociated` event on success
        #[pallet::call_index(93)]
        #[pallet::weight((
            Weight::from_parts(3_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 1)),
            DispatchClass::Operational,
            Pays::Yes
        ))]
        pub fn associate_evm_key(
            origin: T::RuntimeOrigin,
            netuid: NetUid,
            evm_key: H160,
            block_number: u64,
            signature: Signature,
        ) -> DispatchResult {
            Self::do_associate_evm_key(origin, netuid, evm_key, block_number, signature)
        }

        /// Recycles alpha from a cold/hot key pair, reducing AlphaOut on a subnet
        ///
        /// # Arguments
        /// * `origin` - The origin of the call (must be signed by the coldkey)
        /// * `hotkey` - The hotkey account
        /// * `amount` - The amount of alpha to recycle
        /// * `netuid` - The subnet ID
        ///
        /// # Events
        /// Emits a `TokensRecycled` event on success.
        #[pallet::call_index(101)]
        #[pallet::weight((
            Weight::from_parts(101_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(7, 4)),
            DispatchClass::Operational,
            Pays::Yes
        ))]
        pub fn recycle_alpha(
            origin: T::RuntimeOrigin,
            hotkey: T::AccountId,
            amount: AlphaCurrency,
            netuid: NetUid,
        ) -> DispatchResult {
            Self::do_recycle_alpha(origin, hotkey, amount, netuid)
        }

        /// Burns alpha from a cold/hot key pair without reducing `AlphaOut`
        ///
        /// # Arguments
        /// * `origin` - The origin of the call (must be signed by the coldkey)
        /// * `hotkey` - The hotkey account
        /// * `amount` - The amount of alpha to burn
        /// * `netuid` - The subnet ID
        ///
        /// # Events
        /// Emits a `TokensBurned` event on success.
        #[pallet::call_index(102)]
        #[pallet::weight((
            Weight::from_parts(98_010_000, 0).saturating_add(T::DbWeight::get().reads_writes(7, 3)),
            DispatchClass::Operational,
            Pays::Yes
        ))]
        pub fn burn_alpha(
            origin: T::RuntimeOrigin,
            hotkey: T::AccountId,
            amount: AlphaCurrency,
            netuid: NetUid,
        ) -> DispatchResult {
            Self::do_burn_alpha(origin, hotkey, amount, netuid)
        }

        /// Sets the pending childkey cooldown (in blocks). Root only.
        #[pallet::call_index(109)]
        #[pallet::weight((Weight::from_parts(10_000, 0), DispatchClass::Operational, Pays::No))]
        pub fn set_pending_childkey_cooldown(
            origin: OriginFor<T>,
            cooldown: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            PendingChildKeyCooldown::<T>::put(cooldown);
            Ok(())
        }

        /// Removes all stake from a hotkey on a subnet with a price limit.
        /// This extrinsic allows to specify the limit price for alpha token
        /// at which or better (higher) the staking should execute.
        /// Without limit_price it remove all the stake similar to `remove_stake` extrinsic
        #[pallet::call_index(103)]
        #[pallet::weight((Weight::from_parts(398_000_000, 10142)
			.saturating_add(T::DbWeight::get().reads(30_u64))
			.saturating_add(T::DbWeight::get().writes(14_u64)), DispatchClass::Normal, Pays::Yes))]
        pub fn remove_stake_full_limit(
            origin: T::RuntimeOrigin,
            hotkey: T::AccountId,
            netuid: NetUid,
            limit_price: Option<u64>,
        ) -> DispatchResult {
            Self::do_remove_stake_full_limit(origin, hotkey, netuid, limit_price)
        }

        /// Register a new leased network.
        ///
        /// The crowdloan's contributions are used to compute the share of the emissions that the contributors
        /// will receive as dividends.
        ///
        /// The leftover cap is refunded to the contributors and the beneficiary.
        ///
        /// # Args:
        /// * `origin` - (<T as frame_system::Config>::Origin):
        ///     - The signature of the caller's coldkey.
        ///
        /// * `emissions_share` (Percent):
        ///     - The share of the emissions that the contributors will receive as dividends.
        ///
        /// * `end_block` (Option<BlockNumberFor<T>>):
        ///     - The block at which the lease will end. If not defined, the lease is perpetual.
        #[pallet::call_index(110)]
        #[pallet::weight(SubnetLeasingWeightInfo::<T>::do_register_leased_network(T::MaxContributors::get()))]
        pub fn register_leased_network(
            origin: T::RuntimeOrigin,
            emissions_share: Percent,
            end_block: Option<BlockNumberFor<T>>,
        ) -> DispatchResultWithPostInfo {
            Self::do_register_leased_network(origin, emissions_share, end_block)
        }

        /// Terminate a lease.
        ///
        /// The beneficiary can terminate the lease after the end block has passed and get the subnet ownership.
        /// The subnet is transferred to the beneficiary and the lease is removed from storage.
        ///
        /// **The hotkey must be owned by the beneficiary coldkey.**
        ///
        /// # Args:
        /// * `origin` - (<T as frame_system::Config>::Origin):
        ///     - The signature of the caller's coldkey.
        ///
        /// * `lease_id` (LeaseId):
        ///     - The ID of the lease to terminate.
        ///
        /// * `hotkey` (T::AccountId):
        ///     - The hotkey of the beneficiary to mark as subnet owner hotkey.
        #[pallet::call_index(111)]
        #[pallet::weight(SubnetLeasingWeightInfo::<T>::do_terminate_lease(T::MaxContributors::get()))]
        pub fn terminate_lease(
            origin: T::RuntimeOrigin,
            lease_id: LeaseId,
            hotkey: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            Self::do_terminate_lease(origin, lease_id, hotkey)
        }

        /// Updates the symbol for a subnet.
        ///
        /// # Arguments
        /// * `origin` - The origin of the call, which must be the subnet owner or root.
        /// * `netuid` - The unique identifier of the subnet on which the symbol is being set.
        /// * `symbol` - The symbol to set for the subnet.
        ///
        /// # Errors
        /// Returns an error if:
        /// * The transaction is not signed by the subnet owner.
        /// * The symbol does not exist.
        /// * The symbol is already in use by another subnet.
        ///
        /// # Events
        /// Emits a `SymbolUpdated` event on success.
        #[pallet::call_index(112)]
        #[pallet::weight((
            Weight::from_parts(28_840_000, 0).saturating_add(T::DbWeight::get().reads_writes(4, 1)),
            DispatchClass::Operational,
            Pays::Yes
        ))]
        pub fn update_symbol(
            origin: OriginFor<T>,
            netuid: NetUid,
            symbol: Vec<u8>,
        ) -> DispatchResult {
            Self::ensure_subnet_owner_or_root(origin, netuid)?;

            Self::ensure_symbol_exists(&symbol)?;
            Self::ensure_symbol_available(&symbol)?;

            TokenSymbol::<T>::insert(netuid, symbol.clone());

            Self::deposit_event(Event::SymbolUpdated { netuid, symbol });
            Ok(())
        }
    }
}
