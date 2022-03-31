#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{Currency, ReservableCurrency};
use scale_info::TypeInfo;
use sp_runtime::traits::Scale;
use sp_std::{collections::btree_set::BTreeSet, prelude::*};

use pallet_dapi_staking::Staking;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Blockchain id, e.g `eth.mainnet`
	type ChainId<T> = BoundedVec<u8, <T as Config>::MaxBytesInChainId>;
	/// Massbit external UUID type
	type MassbitId = BoundedVec<u8, ConstU32<64>>;

	#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub struct Project<AccountId, ChainId> {
		pub owner: AccountId,
		pub chain_id: ChainId,
		pub quota: u128,
		pub usage: u128,
	}

	#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub enum ProviderType {
		Gateway,
		Node,
	}

	#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub enum ProviderState {
		Registered,
		Unregistered,
	}

	#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub struct Provider<AccountId, ChainId> {
		pub provider_type: ProviderType,
		pub operator: AccountId,
		pub chain_id: ChainId,
		pub state: ProviderState,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Currency: ReservableCurrency<Self::AccountId>;

		/// Interface of dapi staking pallet.
		type StakingInterface: Staking<Self::AccountId, MassbitId, BalanceOf<Self>>;

		/// The origin which can add an oracle.
		type AddOracleOrigin: EnsureOrigin<Self::Origin>;

		/// The origin which can add an fisherman.
		type AddFishermanOrigin: EnsureOrigin<Self::Origin>;

		/// For constraining the maximum bytes of a chain id
		type MaxBytesInChainId: Get<u32>;
	}

	#[pallet::error]
	pub enum Error<T> {
		AlreadyRegistered,
		InsufficientBoding,
		ProjectDNE,
		NotOracle,
		NotFisherman,
		ProviderNotExist,
		NotOwnedProvider,
		NotOperatedProvider,
		InvalidChainId,
		AlreadyCreatedChainId,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A project is successfully registered. \[project_id, account_id, blockchain, quota\]
		ProjectRegistered(MassbitId, T::AccountId, ChainId<T>, u128),
		/// A provider is successfully registered. \[provider_id, provider_type, operator,
		/// blockchain\]
		ProviderRegistered(MassbitId, ProviderType, T::AccountId, ChainId<T>),
		/// A provider is unregistered. \[project_id, account_id, blockchain, quota\]
		ProviderUnregistered(MassbitId, ProviderType),
		/// Project usage is reported by oracle. \[project_id, usage\]
		ProjectUsageReported(MassbitId, u128),
		/// Project reached quota. \[project_id\]
		ProjectReachedQuota(MassbitId),
		/// Provider performance is reported by fisherman. [\provider_id, provider_type, requests,
		/// success_rate, average_latency\]
		ProviderPerformanceReported(MassbitId, ProviderType, u64, u32, u32),
		/// New chain id is created
		ChainIdCreated(ChainId<T>),
	}

	#[pallet::storage]
	#[pallet::getter(fn projects)]
	pub(super) type Projects<T: Config> =
		StorageMap<_, Blake2_128Concat, MassbitId, Project<AccountIdOf<T>, ChainId<T>>>;

	#[pallet::storage]
	#[pallet::getter(fn providers)]
	pub(super) type Providers<T: Config> =
		StorageMap<_, Blake2_128Concat, MassbitId, Provider<AccountIdOf<T>, ChainId<T>>>;

	/// The set of fishermen.
	#[pallet::storage]
	#[pallet::getter(fn fishermen)]
	pub type Fishermen<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

	/// The set of oracles.
	#[pallet::storage]
	#[pallet::getter(fn oracles)]
	pub type Oracles<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

	/// The set of blockchain id.
	#[pallet::storage]
	#[pallet::getter(fn chain_ids)]
	pub type ChainIds<T: Config> = StorageValue<_, BTreeSet<ChainId<T>>, ValueQuery>;

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
			project_id: MassbitId,
			chain_id: ChainId<T>,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(!<Projects<T>>::contains_key(&project_id), Error::<T>::AlreadyRegistered);
			ensure!(Self::is_valid_chain_id(&chain_id), Error::<T>::InvalidChainId);

			T::Currency::reserve(&account, deposit)?;

			let quota = Self::calculate_quota(deposit);
			<Projects<T>>::insert(
				&project_id,
				Project { owner: account.clone(), chain_id: chain_id.clone(), quota, usage: 0 },
			);

			Self::deposit_event(Event::ProjectRegistered(project_id, account, chain_id, quota));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_project_usage(
			origin: OriginFor<T>,
			project_id: MassbitId,
			usage: u128,
		) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;
			ensure!(Self::is_oracle(&account_id), Error::<T>::NotOracle);

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::ProjectDNE)?;
			project.usage = project.usage.saturating_add(usage).min(project.quota);
			if project.usage == project.quota {
				Self::deposit_event(Event::ProjectReachedQuota(project_id.clone()));
			} else {
				Self::deposit_event(Event::ProjectUsageReported(project_id.clone(), usage));
			}

			Projects::<T>::insert(&project_id, project);

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn register_provider(
			origin: OriginFor<T>,
			provider_id: MassbitId,
			provider_type: ProviderType,
			chain_id: ChainId<T>,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let operator = ensure_signed(origin)?;

			ensure!(!<Providers<T>>::contains_key(&provider_id), Error::<T>::AlreadyRegistered);
			ensure!(Self::is_valid_chain_id(&chain_id), Error::<T>::InvalidChainId);

			T::StakingInterface::register(operator.clone(), provider_id.clone(), deposit)?;

			<Providers<T>>::insert(
				&provider_id,
				Provider {
					provider_type,
					operator: operator.clone(),
					chain_id: chain_id.clone(),
					state: ProviderState::Registered,
				},
			);

			Self::deposit_event(Event::ProviderRegistered(
				provider_id,
				provider_type,
				operator,
				chain_id,
			));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn unregister_provider(
			origin: OriginFor<T>,
			provider_id: MassbitId,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			let mut provider =
				Providers::<T>::get(&provider_id).ok_or(Error::<T>::ProviderNotExist)?;
			ensure!(provider.operator == account, Error::<T>::NotOwnedProvider);

			ensure!(provider.state == ProviderState::Registered, Error::<T>::NotOperatedProvider);

			T::StakingInterface::unregister(provider_id.clone())?;

			provider.state = ProviderState::Unregistered;
			Providers::<T>::insert(&provider_id, provider.clone());

			Self::deposit_event(Event::<T>::ProviderUnregistered(
				provider_id,
				provider.provider_type,
			));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_provider_report(
			origin: OriginFor<T>,
			provider_id: MassbitId,
			requests: u64,
			success_percentage: u32,
			average_latency: u32,
		) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;
			ensure!(Self::is_fisherman(&account_id), Error::<T>::NotFisherman);

			let mut provider = Self::providers(&provider_id).ok_or(Error::<T>::ProviderNotExist)?;
			ensure!(provider.state == ProviderState::Registered, Error::<T>::NotOperatedProvider);

			T::StakingInterface::unregister(provider_id.clone())?;

			provider.state = ProviderState::Unregistered;
			Providers::<T>::insert(&provider_id, provider.clone());

			Self::deposit_event(Event::ProviderPerformanceReported(
				provider_id,
				provider.provider_type,
				requests,
				success_percentage,
				average_latency,
			));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn create_chain_id(
			origin: OriginFor<T>,
			chain_id: ChainId<T>,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_root(origin);

			ensure!(!Self::is_valid_chain_id(&chain_id), Error::<T>::AlreadyCreatedChainId);

			ChainIds::<T>::mutate(|chain_ids| chain_ids.insert(chain_id.clone()));

			Self::deposit_event(Event::ChainIdCreated(chain_id));

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		fn calculate_quota(amount: BalanceOf<T>) -> u128 {
			TryInto::<u128>::try_into(amount)
				.ok()
				.unwrap_or_default()
				.div(1_000_000_000_000_000u128)
		}

		fn is_valid_chain_id(chain_id: &ChainId<T>) -> bool {
			Self::chain_ids().iter().any(|id| id == chain_id)
		}

		fn is_fisherman(account_id: &T::AccountId) -> bool {
			Self::fishermen().iter().any(|id| id == account_id)
		}

		fn initialize_fishermen(fishermen: &Vec<T::AccountId>) {
			let fishermen_ids = fishermen
				.iter()
				.map(|fisherman| fisherman.clone())
				.collect::<BTreeSet<T::AccountId>>();
			Fishermen::<T>::put(&fishermen_ids);
		}

		fn is_oracle(account_id: &T::AccountId) -> bool {
			Self::oracles().iter().any(|id| id == account_id)
		}

		fn initialize_oracles(oracles: &Vec<T::AccountId>) {
			let oracle_ids =
				oracles.iter().map(|oracle| oracle.clone()).collect::<BTreeSet<T::AccountId>>();
			Oracles::<T>::put(&oracle_ids);
		}
	}
}
