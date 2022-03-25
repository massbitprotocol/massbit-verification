#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::Currency;
use scale_info::TypeInfo;
use sp_std::{collections::btree_set::BTreeSet, prelude::*};

use pallet_dapi_staking::Staking;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::ReservableCurrency};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Scale;

	type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	type ProviderId = [u8; 36];
	type BlockChain = Vec<u8>;

	#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub struct Project<AccountId> {
		pub owner: AccountId,
		pub blockchain: BlockChain,
		pub quota: u128,
		pub usage: u128,
	}

	#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub enum ProviderType {
		Gateway,
		Node,
	}

	#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub struct Provider<AccountId> {
		pub provider_type: ProviderType,
		pub operator: AccountId,
		pub blockchain: BlockChain,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Currency: ReservableCurrency<Self::AccountId>;

		type StakingInterface: Staking<Self::AccountId, ProviderId>;

		/// The origin which can add an oracle.
		type AddOracleOrigin: EnsureOrigin<Self::Origin>;

		/// The origin which can add an fisherman.
		type AddFishermanOrigin: EnsureOrigin<Self::Origin>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::error]
	pub enum Error<T> {
		AlreadyRegistered,
		InsufficientBoding,
		ProjectNotFound,
		NotOracle,
		NotFisherman,
		ProviderNotExist,
		NotOwnedProvider,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A project is successfully registered. \[project_id, account_id, blockchain, quota\]
		ProjectRegistered(ProviderId, T::AccountId, BlockChain, u128),
		/// A provider is successfully registered. \[provider_id, provider_type, operator,
		/// blockchain\]
		ProviderRegistered(ProviderId, ProviderType, T::AccountId, BlockChain),
		/// A provider is unregistered. \[project_id, account_id, blockchain, quota\]
		ProviderUnregistered(ProviderId, ProviderType),
		/// Project usage is reported.
		ProjectUsageReported(ProviderId, u128),
		/// Provide performance is reported.
		ProviderPerformanceReported(ProviderId, ProviderType, u64, u32, u32),
	}

	#[pallet::storage]
	#[pallet::getter(fn projects)]
	pub(super) type Projects<T: Config> =
		StorageMap<_, Blake2_128Concat, ProviderId, Project<AccountIdOf<T>>>;

	#[pallet::storage]
	#[pallet::getter(fn providers)]
	pub(super) type Providers<T: Config> =
		StorageMap<_, Blake2_128Concat, ProviderId, Provider<AccountIdOf<T>>>;

	/// The set of fishermen.
	#[pallet::storage]
	#[pallet::getter(fn fishermen)]
	pub type Fishermen<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

	/// The set of oracles.
	#[pallet::storage]
	#[pallet::getter(fn oracles)]
	pub type Oracles<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub fishermen: Vec<T::AccountId>,
		pub oracles: Vec<T::AccountId>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { fishermen: Vec::new(), oracles: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_fishermen(&self.fishermen);
			Pallet::<T>::initialize_oracles(&self.oracles);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn register_project(
			origin: OriginFor<T>,
			project_id: ProviderId,
			blockchain: BlockChain,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(<Projects<T>>::get(&project_id).is_none(), Error::<T>::AlreadyRegistered);

			T::Currency::reserve(&account, deposit)?;

			let quota = Self::calculate_consumer_quota(deposit);
			<Projects<T>>::insert(
				&project_id,
				Project { owner: account.clone(), blockchain: blockchain.clone(), quota, usage: 0 },
			);

			Self::deposit_event(Event::ProjectRegistered(project_id, account, blockchain, quota));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn register_provider(
			origin: OriginFor<T>,
			provider_id: ProviderId,
			provider_type: ProviderType,
			blockchain: BlockChain,
		) -> DispatchResultWithPostInfo {
			let operator = ensure_signed(origin)?;

			ensure!(<Providers<T>>::get(&provider_id).is_none(), Error::<T>::AlreadyRegistered);

			T::StakingInterface::register(operator.clone(), provider_id.clone())?;

			<Providers<T>>::insert(
				&provider_id,
				Provider {
					provider_type,
					operator: operator.clone(),
					blockchain: blockchain.clone(),
				},
			);

			Self::deposit_event(Event::ProviderRegistered(
				provider_id,
				provider_type,
				operator,
				blockchain,
			));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn unregister_provider(
			origin: OriginFor<T>,
			provider_id: ProviderId,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			let provider = Providers::<T>::get(&provider_id).ok_or(Error::<T>::ProviderNotExist)?;
			ensure!(provider.operator == account, Error::<T>::NotOwnedProvider);

			T::StakingInterface::unregister(provider_id.clone())?;

			Self::deposit_event(Event::<T>::ProviderUnregistered(
				provider_id,
				provider.provider_type,
			));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_project_usage(
			origin: OriginFor<T>,
			project_id: ProviderId,
			usage: u128,
		) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;

			ensure!(Self::is_oracle(&account_id), Error::<T>::NotOracle);

			Projects::<T>::mutate(&project_id, |project| {
				if let Some(project) = project {
					project.usage = project.usage.saturating_add(usage)
				}
			});

			Self::deposit_event(Event::ProjectUsageReported(project_id, usage));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_provider_report(
			origin: OriginFor<T>,
			provider_id: ProviderId,
			requests: u64,
			success_percentage: u32,
			average_latency: u32,
		) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;

			ensure!(Self::is_fisherman(&account_id), Error::<T>::NotFisherman);

			let provider = Self::providers(&provider_id).ok_or(Error::<T>::ProviderNotExist)?;

			T::StakingInterface::unregister(provider_id.clone())?;

			Self::deposit_event(Event::ProviderPerformanceReported(
				provider_id,
				provider.provider_type,
				requests,
				success_percentage,
				average_latency,
			));

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		fn calculate_consumer_quota(amount: BalanceOf<T>) -> u128 {
			TryInto::<u128>::try_into(amount)
				.ok()
				.unwrap_or_default()
				.div(1_000_000_000_000_000u128)
		}

		fn is_fisherman(account_id: &T::AccountId) -> bool {
			Self::fishermen().iter().any(|id| id == account_id)
		}

		fn is_oracle(account_id: &T::AccountId) -> bool {
			Self::oracles().iter().any(|id| id == account_id)
		}

		fn initialize_fishermen(fishermen: &Vec<T::AccountId>) {
			let fishermen_ids = fishermen
				.iter()
				.map(|fisherman| fisherman.clone())
				.collect::<BTreeSet<T::AccountId>>();
			Fishermen::<T>::put(&fishermen_ids);
		}

		fn initialize_oracles(oracles: &Vec<T::AccountId>) {
			let oracle_ids =
				oracles.iter().map(|oracle| oracle.clone()).collect::<BTreeSet<T::AccountId>>();
			Oracles::<T>::put(&oracle_ids);
		}
	}
}
