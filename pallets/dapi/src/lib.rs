#![cfg_attr(not(feature = "std"), no_std)]

pub mod types;
pub mod weights;

use frame_support::traits::{Currency, ReservableCurrency};
use sp_runtime::traits::{Scale, Zero};
use sp_std::{collections::btree_set::BTreeSet, prelude::*};

use pallet_dapi_staking::Staking;

#[cfg(any(feature = "runtime-benchmarks"))]
pub mod benchmarking;
#[cfg(test)]
mod mock;

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Blockchain identifier, e.g `eth.mainnet`
	type ChainId<T> = BoundedVec<u8, <T as Config>::StringLimit>;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency mechanism
		type Currency: ReservableCurrency<Self::AccountId>;

		/// Interface of dapi staking pallet.
		type StakingInterface: Staking<Self::AccountId, Self::MassbitId, BalanceOf<Self>>;

		/// The origin which can add an fisherman.
		type AddFishermanOrigin: EnsureOrigin<Self::Origin>;

		/// For constraining the maximum length of a chain id.
		type StringLimit: Get<u32>;

		/// The identifier of Massbit provider/project.
		type MassbitId: Parameter + Member + Default;

		/// Max number of deposit chunks per account Id <-> project Id pairing.
		/// If value is zero, deposit becomes impossible.
		#[pallet::constant]
		type MaxDepositChunks: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		AlreadyRegistered,
		ProjectDNE,
		NotOwnedProject,
		TooManyDepositChunks,
		NothingToWithdraw,
		ProviderDNE,
		NotOwnedProvider,
		NotOperatedProvider,
		InvalidChainId,
		NotFisherman,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A project is registered.
		ProjectRegistered {
			project_id: T::MassbitId,
			consumer: T::AccountId,
			chain_id: ChainId<T>,
			quota: u128,
		},
		/// A project is deposited more.
		ProjectDeposited { project_id: T::MassbitId, consumer: T::AccountId, quota: u128 },
		/// A provider is registered.
		ProviderRegistered {
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			operator: T::AccountId,
			chain_id: ChainId<T>,
		},
		/// A provider is unregistered.
		ProviderUnregistered { provider_id: T::MassbitId, provider_type: ProviderType },
		/// Project usage is reported.
		ProjectUsageReported { provider_id: T::MassbitId, usage: u128 },
		/// Project reached max quota.
		ProjectReachedQuota { project_id: T::MassbitId },
		/// Provider performance is reported.
		ProviderPerformanceReported {
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			requests: u64,
			success_rate: u32,
			average_latency: u32,
		},
		/// New chain id is created.
		ChainIdCreated { chain_id: BoundedVec<u8, T::StringLimit> },
		/// Account has withdrawn unbonded funds.
		Withdrawn { account: T::AccountId, amount: BalanceOf<T> },
	}

	#[pallet::storage]
	#[pallet::getter(fn projects)]
	pub(super) type Projects<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::MassbitId,
		Project<AccountIdOf<T>, ChainId<T>, BalanceOf<T>, T::BlockNumber>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn providers)]
	pub(super) type Providers<T: Config> =
		StorageMap<_, Blake2_128Concat, T::MassbitId, Provider<AccountIdOf<T>, ChainId<T>>>;

	/// The set of fishermen.
	#[pallet::storage]
	#[pallet::getter(fn fishermen)]
	pub type Fishermen<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

	/// The set of blockchain id.
	#[pallet::storage]
	#[pallet::getter(fn chain_ids)]
	pub type ChainIds<T: Config> = StorageValue<_, BTreeSet<ChainId<T>>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub fishermen: Vec<T::AccountId>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { fishermen: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_fishermen(&self.fishermen);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::register_project())]
		pub fn register_project(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			chain_id: Vec<u8>,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(!<Projects<T>>::contains_key(&project_id), Error::<T>::AlreadyRegistered);

			let chain_id: BoundedVec<u8, T::StringLimit> =
				chain_id.try_into().map_err(|_| Error::<T>::InvalidChainId)?;
			ensure!(Self::is_valid_chain_id(&chain_id), Error::<T>::InvalidChainId);

			T::Currency::reserve(&account, deposit)?;

			let quota = Self::calculate_quota(deposit);
			let mut project = Project {
				consumer: account.clone(),
				chain_id: chain_id.clone(),
				quota,
				usage: 0,
				deposit_info: Default::default(),
			};
			project.deposit_info.add(Deposit {
				amount: deposit,
				unreserved_block_number: <frame_system::Pallet<T>>::block_number(),
			});

			<Projects<T>>::insert(&project_id, project);

			Self::deposit_event(Event::ProjectRegistered {
				project_id,
				consumer: account,
				chain_id,
				quota,
			});

			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::deposit_project())]
		pub fn deposit_project(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let consumer = ensure_signed(origin)?;

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::ProjectDNE)?;
			ensure!(project.consumer == consumer, Error::<T>::NotOwnedProject);
			ensure!(
				project.deposit_info.len() <= T::MaxDepositChunks::get(),
				Error::<T>::TooManyDepositChunks
			);

			T::Currency::reserve(&consumer, deposit)?;

			let quota = project.quota.saturating_add(Self::calculate_quota(deposit));
			project.quota = quota;
			project.deposit_info.add(Deposit {
				amount: deposit,
				unreserved_block_number: <frame_system::Pallet<T>>::block_number(),
			});

			<Projects<T>>::insert(&project_id, project);

			Self::deposit_event(Event::ProjectDeposited { consumer, project_id, quota });

			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::withdraw_project_deposit())]
		pub fn withdraw_project_deposit(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
		) -> DispatchResultWithPostInfo {
			let consumer = ensure_signed(origin)?;

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::ProjectDNE)?;
			ensure!(project.consumer == consumer, Error::<T>::NotOwnedProject);

			let current_block = <frame_system::Pallet<T>>::block_number();
			let (valid_chunks, future_chunks) = project.deposit_info.partition(current_block);
			let withdraw_amount = valid_chunks.sum();
			ensure!(!withdraw_amount.is_zero(), Error::<T>::NothingToWithdraw);

			project.deposit_info = future_chunks;
			Projects::<T>::insert(&project_id, project);

			T::Currency::unreserve(&consumer, withdraw_amount);

			Self::deposit_event(Event::<T>::Withdrawn {
				account: consumer,
				amount: withdraw_amount,
			});

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_project_usage(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			usage: u128,
		) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;
			ensure!(Self::is_fisherman(&account_id), Error::<T>::NotFisherman);

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::ProjectDNE)?;
			project.usage = project.usage.saturating_add(usage).min(project.quota);
			if project.usage == project.quota {
				Self::deposit_event(Event::ProjectReachedQuota { project_id: project_id.clone() });
			} else {
				Self::deposit_event(Event::ProjectUsageReported {
					provider_id: project_id.clone(),
					usage,
				});
			}

			Projects::<T>::insert(&project_id, project);

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn register_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			chain_id: Vec<u8>,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let operator = ensure_signed(origin)?;

			ensure!(!<Providers<T>>::contains_key(&provider_id), Error::<T>::ProviderDNE);

			let chain_id: BoundedVec<u8, T::StringLimit> =
				chain_id.try_into().map_err(|_| Error::<T>::InvalidChainId)?;
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

			Self::deposit_event(Event::ProviderRegistered {
				provider_id,
				provider_type,
				operator,
				chain_id,
			});

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn unregister_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			let mut provider = Providers::<T>::get(&provider_id).ok_or(Error::<T>::ProviderDNE)?;
			ensure!(provider.operator == account, Error::<T>::NotOwnedProvider);

			ensure!(provider.state == ProviderState::Registered, Error::<T>::NotOperatedProvider);

			T::StakingInterface::unregister(provider_id.clone())?;

			provider.state = ProviderState::Unregistered;
			Providers::<T>::insert(&provider_id, provider.clone());

			Self::deposit_event(Event::<T>::ProviderUnregistered {
				provider_id,
				provider_type: provider.provider_type,
			});

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_provider_report(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			requests: u64,
			success_rate: u32,
			average_latency: u32,
		) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;
			ensure!(Self::is_fisherman(&account_id), Error::<T>::NotFisherman);

			let mut provider = Self::providers(&provider_id).ok_or(Error::<T>::ProviderDNE)?;
			ensure!(provider.state == ProviderState::Registered, Error::<T>::NotOperatedProvider);

			T::StakingInterface::unregister(provider_id.clone())?;

			provider.state = ProviderState::Unregistered;
			Providers::<T>::insert(&provider_id, provider.clone());

			Self::deposit_event(Event::ProviderPerformanceReported {
				provider_id,
				provider_type: provider.provider_type,
				requests,
				success_rate,
				average_latency,
			});

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn create_chain_id(
			origin: OriginFor<T>,
			chain_id: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_root(origin);

			let chain_id: BoundedVec<u8, T::StringLimit> =
				chain_id.try_into().map_err(|_| Error::<T>::InvalidChainId)?;
			ensure!(!Self::is_valid_chain_id(&chain_id), Error::<T>::AlreadyRegistered);

			ChainIds::<T>::mutate(|chain_ids| chain_ids.insert(chain_id.clone()));

			Self::deposit_event(Event::ChainIdCreated { chain_id });

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
	}
}
