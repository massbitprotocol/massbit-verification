use super::*;
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::*,
	traits::{
		Currency, ExistenceRequirement, Get, Imbalance, LockIdentifier, LockableCurrency,
		OnUnbalanced, ReservableCurrency, WithdrawReasons,
	},
	weights::Weight,
	PalletId,
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, Saturating, Zero},
	ArithmeticError, Perbill,
};
use sp_std::convert::From;

const STAKING_ID: LockIdentifier = *b"apistake";

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	impl<T: Config> OnUnbalanced<NegativeImbalanceOf<T>> for Pallet<T> {
		fn on_nonzero_unbalanced(block_reward: NegativeImbalanceOf<T>) {
			BlockRewardAccumulator::<T>::mutate(|accumulated_reward| {
				*accumulated_reward = accumulated_reward.saturating_add(block_reward.peek())
			});
			T::Currency::resolve_creating(&Self::account_id(), block_reward);
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// DapiPool contains gateways/nodes run a blockchain network in a zone.
		type DapiPool: Parameter + Member;

		/// The staking balance.
		type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>
			+ ReservableCurrency<Self::AccountId>;

		/// Number of block per era.
		#[pallet::constant]
		type BlockPerEra: Get<BlockNumberFor<Self>>;

		/// Number of eras that are valid when claiming rewards.
		///
		/// All the rest will be either claimed by the treasury or discarded.
		#[pallet::constant]
		type HistoryDepth: Get<u32>;

		/// Number of eras that need to pass until unstaked value can be withdrawn.
		/// Current era is always counted as full era (regardless how much blocks are remaining).
		/// When set to `0`, it's equal to having no unbonding period.
		#[pallet::constant]
		type UnbondingPeriod: Get<u32>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Minimum amount that should be left on staker account after staking.
		#[pallet::constant]
		type MinimumRemainingAmount: Get<BalanceOf<Self>>;
	}

	/// Bonded amount for the staker.
	#[pallet::storage]
	#[pallet::getter(fn ledger)]
	pub type Ledger<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, AccountLedger<BalanceOf<T>>, ValueQuery>;

	/// The current era index.
	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	pub type CurrentEra<T> = StorageValue<_, EraIndex, ValueQuery>;

	/// Accumulator for block rewards during an era. It is reset at every new era
	#[pallet::storage]
	#[pallet::getter(fn block_reward_accumulator)]
	pub type BlockRewardAccumulator<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Total block rewards for the pallet per era and total staked funds.
	#[pallet::storage]
	#[pallet::getter(fn era_reward_and_stake)]
	pub type EraRewardsAndStakes<T: Config> =
		StorageMap<_, Twox64Concat, EraIndex, EraRewardAndStake<BalanceOf<T>>>;

	/// Stores amount staked and stakers for a dapi pool per era
	#[pallet::storage]
	#[pallet::getter(fn pool_era_stake)]
	pub type PoolEraStake<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::DapiPool,
		Twox64Concat,
		EraIndex,
		EraStakingPoints<T::AccountId, BalanceOf<T>>,
	>;

	#[pallet::type_value]
	pub fn ForceEraOnEmpty() -> Forcing {
		Forcing::ForceNone
	}

	/// Mode of era forcing.
	#[pallet::storage]
	#[pallet::getter(fn force_era)]
	pub type ForceEra<T> = StorageValue<_, Forcing, ValueQuery, ForceEraOnEmpty>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Account has bonded and staked funds on a smart contract.
		BondAndStake(T::AccountId, T::DapiPool, BalanceOf<T>),
		/// New dapi staking era. Distribute era rewards to pools.
		NewDapiStakingEra(EraIndex),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Can not stake with zero value.
		StakingWithNoValue,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: BlockNumberFor<T>) -> Weight {
			let force_new_era = Self::force_era().eq(&Forcing::ForceNew);
			let block_per_era = T::BlockPerEra::get();
			let previous_era = Self::current_era();

			// Value is compared to 1 since genesis block is ignored
			if now % block_per_era == BlockNumberFor::<T>::from(1u32) ||
				force_new_era || previous_era.is_zero()
			{
				let next_era = previous_era + 1;
				CurrentEra::<T>::put(next_era);

				let reward = BlockRewardAccumulator::<T>::take();
				Self::reward_balance_snapshot(previous_era, reward);

				if force_new_era {
					ForceEra::<T>::put(Forcing::ForceNone);
				}

				Self::deposit_event(Event::<T>::NewDapiStakingEra(next_era));
			}

			T::DbWeight::get().writes(5)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Lock up and stake balance of the origin account.
		///
		/// `value` must be more than the `minimum_balance` specified by `T::Currency`
		/// unless account already has bonded value equal or more than 'minimum_balance'.
		///
		/// Effects of staking will be felt at the beginning of the next era.
		#[pallet::weight(100)]
		pub fn bond_and_stake(
			origin: OriginFor<T>,
			pool_id: T::DapiPool,
			#[pallet::compact] value: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let staker = ensure_signed(origin)?;

			// Get the staking ledger or create an entry if it doesn't exist.
			let mut ledger = Self::ledger(&staker);

			// Ensure that staker has enough balance to bond & stake.
			let free_balance =
				T::Currency::free_balance(&staker).saturating_sub(T::MinimumRemainingAmount::get());

			// Remove already locked funds from the free balance
			let available_balance = free_balance.saturating_sub(ledger.locked);
			let value_to_stake = value.min(available_balance);
			ensure!(value_to_stake > Zero::zero(), Error::<T>::StakingWithNoValue);

			// Get the latest era staking point info or create it if pool hasn't been staked yet.
			let current_era = Self::current_era();
			let mut staking_info = Self::staking_info(&pool_id, current_era);

			// Increment ledger and total staker value for pool. Overflow shouldn't be possible but
			// the check is here just for safety.
			ledger.locked =
				ledger.locked.checked_add(&value_to_stake).ok_or(ArithmeticError::Overflow)?;
			staking_info.total = staking_info
				.total
				.checked_add(&value_to_stake)
				.ok_or(ArithmeticError::Overflow)?;

			// Increment staker's staking amount
			let entry = staking_info.stakers.entry(staker.clone()).or_default();
			*entry = entry.checked_add(&value_to_stake).ok_or(ArithmeticError::Overflow)?;

			// Update total staked value in era.
			EraRewardsAndStakes::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_add(value_to_stake);
				}
			});

			// Update ledger and payee
			Self::update_ledger(&staker, ledger);

			// Update staked information for pool in current era
			PoolEraStake::<T>::insert(pool_id.clone(), current_era, staking_info);

			Self::deposit_event(Event::<T>::BondAndStake(staker, pool_id, value_to_stake));

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get AccountId of the pallet
		fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		/// Update the ledger for a staker. This will also update the stash lock.
		/// This lock will lock the entire funds except paying for further transactions.
		fn update_ledger(staker: &T::AccountId, ledger: AccountLedger<BalanceOf<T>>) {
			if ledger.locked.is_zero() && ledger.unbonding_info.is_empty() {
				Ledger::<T>::remove(&staker);
				T::Currency::remove_lock(STAKING_ID, &staker);
			} else {
				T::Currency::set_lock(STAKING_ID, &staker, ledger.locked, WithdrawReasons::all());
				Ledger::<T>::insert(staker, ledger);
			}
		}

		/// The block rewards are accumulated on the pallet's account during an era.
		/// This function takes a snapshot of the pallet's balance accrued during current era
		/// and stores it for future distribution
		///
		/// This is called just at the beginning of an era.
		fn reward_balance_snapshot(era: EraIndex, reward: BalanceOf<T>) {
			// Get the reward and stake information for previous era
			let mut reward_and_stake = Self::era_reward_and_stake(era).unwrap_or_default();

			// Prepare info for the next era
			EraRewardsAndStakes::<T>::insert(
				era + 1,
				EraRewardAndStake {
					rewards: Zero::zero(),
					staked: reward_and_stake.staked.clone(),
				},
			);

			// Set the reward for the previous era.
			reward_and_stake.rewards = reward;
			EraRewardsAndStakes::<T>::insert(era, reward_and_stake);
		}

		/// Returns `EraStakingPoints` for given era if possible or latest stored data or finally
		/// default value if storage have no data for it.
		pub fn staking_info(
			pool_id: &T::DapiPool,
			era: EraIndex,
		) -> EraStakingPoints<T::AccountId, BalanceOf<T>> {
			if let Some(staking_info) = PoolEraStake::<T>::get(pool_id, era) {
				staking_info
			} else {
				let available_era = PoolEraStake::<T>::iter_key_prefix(&pool_id)
					.filter(|x| *x <= era)
					.max()
					.unwrap_or(Zero::zero());
				let mut staking_points =
					PoolEraStake::<T>::get(pool_id, available_era).unwrap_or_default();
				staking_points.claimed_rewards = Zero::zero();
				staking_points
			}
		}
	}
}
