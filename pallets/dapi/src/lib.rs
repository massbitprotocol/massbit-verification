#![cfg_attr(not(feature = "std"), no_std)]

pub mod types;
pub mod weights;

use frame_support::traits::{
	Currency, ExistenceRequirement, OnUnbalanced, ReservableCurrency, WithdrawReasons,
};
use sp_runtime::traits::Scale;
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
	type ChainId<T> = BoundedVec<u8, <T as Config>::ChainIdMaxLength>;

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

		/// The origin which can add fisherman.
		type AddFishermanOrigin: EnsureOrigin<Self::Origin>;

		/// For constraining the maximum length of a chain id.
		type ChainIdMaxLength: Get<u32>;

		/// The identifier of Massbit provider/project.
		type MassbitId: Parameter + Member + Default;

		/// Handle project payment as imbalance.
		type OnProjectPayment: OnUnbalanced<
			<Self::Currency as Currency<Self::AccountId>>::NegativeImbalance,
		>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Entity is already registered.
		AlreadyExist,
		/// The provider is inactive.
		NotOperatedProvider,
		/// Chain Id is too long.
		BadChainId,
		/// The provider or project doesn't exist in the list.
		NotExist,
		/// You are not the owner of the provider or project.
		NotOwner,
		/// No permission to perform specific operation.
		PermissionDenied,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A project is registered.
		ProjectRegistered {
			project_id: T::MassbitId,
			consumer: T::AccountId,
			chain_id: Vec<u8>,
			quota: u128,
		},
		/// A project is deposited more.
		ProjectDeposited { project_id: T::MassbitId, consumer: T::AccountId, quota: u128 },
		/// A provider is registered.
		ProviderRegistered {
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			operator: T::AccountId,
			chain_id: Vec<u8>,
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
		/// Account has withdrawn unbonded funds.
		Withdrawn { account: T::AccountId, amount: BalanceOf<T> },
		/// Chain Id is added to well known set.
		ChainIdAdded { chain_id: Vec<u8> },
		/// Chain id is removed from well known set.
		ChainIdRemoved { chain_id: Vec<u8> },
		/// Fisherman is added
		FishermanAdded { account_id: T::AccountId },
		/// Fisherman is removed
		FishermanRemoved { account_id: T::AccountId },
	}

	#[pallet::storage]
	#[pallet::getter(fn projects)]
	pub(super) type Projects<T: Config> =
		StorageMap<_, Blake2_128Concat, T::MassbitId, Project<AccountIdOf<T>, ChainId<T>>>;

	#[pallet::storage]
	#[pallet::getter(fn providers)]
	pub(super) type Providers<T: Config> =
		StorageMap<_, Blake2_128Concat, T::MassbitId, Provider<AccountIdOf<T>, ChainId<T>>>;

	#[pallet::storage]
	#[pallet::getter(fn fishermen)]
	pub type Fishermen<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

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
			let consumer = ensure_signed(origin)?;

			ensure!(!<Projects<T>>::contains_key(&project_id), Error::<T>::AlreadyExist);

			let bounded_chain_id: BoundedVec<u8, T::ChainIdMaxLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::BadChainId)?;
			ensure!(Self::chain_ids().contains(&bounded_chain_id), Error::<T>::BadChainId);

			let payment = T::Currency::withdraw(
				&consumer,
				deposit,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::KeepAlive,
			)?;
			T::OnProjectPayment::on_unbalanced(payment);

			let quota = Self::calculate_quota(deposit);
			let project =
				Project { consumer: consumer.clone(), chain_id: bounded_chain_id, quota, usage: 0 };

			<Projects<T>>::insert(&project_id, project);

			Self::deposit_event(Event::ProjectRegistered { project_id, consumer, chain_id, quota });
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::deposit_project())]
		pub fn deposit_project(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let consumer = ensure_signed(origin)?;

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::NotExist)?;
			ensure!(project.consumer == consumer, Error::<T>::NotOwner);

			let payment = T::Currency::withdraw(
				&consumer,
				deposit,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::KeepAlive,
			)?;
			T::OnProjectPayment::on_unbalanced(payment);

			let quota = project.quota.saturating_add(Self::calculate_quota(deposit));
			project.quota = quota;

			<Projects<T>>::insert(&project_id, project);

			Self::deposit_event(Event::ProjectDeposited { consumer, project_id, quota });
			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_project_usage(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			usage: u128,
		) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;
			ensure!(Self::fishermen().contains(&account_id), Error::<T>::PermissionDenied);

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::NotExist)?;
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

		#[pallet::weight(T::WeightInfo::register_provider())]
		pub fn register_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			chain_id: Vec<u8>,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let operator = ensure_signed(origin)?;

			ensure!(!<Providers<T>>::contains_key(&provider_id), Error::<T>::AlreadyExist);

			let bounded_chain_id: BoundedVec<u8, T::ChainIdMaxLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::BadChainId)?;
			ensure!(Self::chain_ids().contains(&bounded_chain_id), Error::<T>::BadChainId);

			T::StakingInterface::register(operator.clone(), provider_id.clone(), deposit)?;

			<Providers<T>>::insert(
				&provider_id,
				Provider {
					provider_type,
					operator: operator.clone(),
					chain_id: bounded_chain_id,
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

		#[pallet::weight(T::WeightInfo::unregister_provider())]
		pub fn unregister_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			let mut provider = Providers::<T>::get(&provider_id).ok_or(Error::<T>::NotExist)?;
			ensure!(provider.operator == account, Error::<T>::NotOwner);

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
			ensure!(Self::fishermen().contains(&account_id), Error::<T>::PermissionDenied);

			let mut provider = Self::providers(&provider_id).ok_or(Error::<T>::NotExist)?;
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

		#[pallet::weight(T::WeightInfo::add_chain_id())]
		pub fn add_chain_id(origin: OriginFor<T>, chain_id: Vec<u8>) -> DispatchResultWithPostInfo {
			let _ = ensure_root(origin);

			let bounded_chain_id: BoundedVec<u8, T::ChainIdMaxLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::BadChainId)?;

			let mut chain_ids = ChainIds::<T>::get();
			ensure!(!chain_ids.contains(&bounded_chain_id), Error::<T>::AlreadyExist);

			chain_ids.insert(bounded_chain_id);
			ChainIds::<T>::put(&chain_ids);

			Self::deposit_event(Event::ChainIdAdded { chain_id });
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::remove_chain_id())]
		pub fn remove_chain_id(
			origin: OriginFor<T>,
			chain_id: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_root(origin);

			let bounded_chain_id: BoundedVec<u8, T::ChainIdMaxLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::BadChainId)?;

			let mut chain_ids = ChainIds::<T>::get();
			ensure!(chain_ids.contains(&bounded_chain_id), Error::<T>::NotExist);

			chain_ids.remove(&bounded_chain_id);
			ChainIds::<T>::put(&chain_ids);

			Self::deposit_event(Event::ChainIdRemoved { chain_id });
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::add_fisherman())]
		pub fn add_fisherman(
			origin: OriginFor<T>,
			account_id: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_root(origin);

			let mut fishermen = Fishermen::<T>::get();
			ensure!(!fishermen.contains(&account_id), Error::<T>::AlreadyExist);

			fishermen.insert(account_id.clone());
			Fishermen::<T>::put(&fishermen);

			Self::deposit_event(Event::FishermanAdded { account_id });
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::remove_fisherman())]
		pub fn remove_fisherman(
			origin: OriginFor<T>,
			account_id: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_root(origin);

			let mut fishermen = Fishermen::<T>::get();
			ensure!(fishermen.contains(&account_id), Error::<T>::NotExist);

			fishermen.remove(&account_id);
			Fishermen::<T>::put(&fishermen);

			Self::deposit_event(Event::FishermanRemoved { account_id });
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

		fn initialize_fishermen(fishermen: &Vec<T::AccountId>) {
			let fishermen_ids = fishermen
				.iter()
				.map(|fisherman| fisherman.clone())
				.collect::<BTreeSet<T::AccountId>>();
			Fishermen::<T>::put(&fishermen_ids);
		}
	}
}
