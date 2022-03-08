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
			todo!()
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}
