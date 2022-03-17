#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use frame_support::serde::{Deserialize, Serialize};
use frame_support::{sp_runtime::traits::Hash, traits::Currency};
use scale_info::TypeInfo;
use sp_runtime::traits::IsMember;
use sp_std::prelude::*;

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

	type MassbitId = Vec<u8>;

	#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum BlockChain {
		Ethereum,
		Polkadot,
	}

	#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub struct Project<AccountId> {
		pub owner: AccountId,
		pub blockchain: BlockChain,
		pub quota: u64,
		pub usage: u64,
	}

	#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub struct Gateway<AccountId, Balance> {
		pub owner: AccountId,
		pub blockchain: BlockChain,
		pub deposit: Balance,
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct Node<AccountId, Balance> {
		pub owner: AccountId,
		pub blockchain: BlockChain,
		pub deposit: Balance,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Currency: Currency<Self::AccountId>;

		type MinGatewayDeposit: Get<BalanceOf<Self>>;

		type MinNodeDeposit: Get<BalanceOf<Self>>;

		type Staking: Staking<BalanceOf<Self>, Self::AccountId, Self::Hash>;

		type IsOracle: IsMember<Self::AccountId>;
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
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A project is successfully registered. \[project_id, account_id, blockchain, quota\]
		ProjectRegistered(MassbitId, T::AccountId, BlockChain, u64),
		/// A gateway is successfully registered. \[gateway_id, account_id, blockchain, deposit\]
		GatewayRegistered(MassbitId, T::AccountId, BlockChain, BalanceOf<T>),
		/// A node is successfully registered. \[node_id, account_id, blockchain, deposit\]
		NodeRegistered(MassbitId, T::AccountId, BlockChain, BalanceOf<T>),
		/// Project usage is reported.
		ProjectUsageReported(MassbitId, u64),
	}

	#[pallet::storage]
	#[pallet::getter(fn consumers)]
	pub(super) type Projects<T: Config> =
		StorageMap<_, Blake2_128Concat, MassbitId, Project<AccountIdOf<T>>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn gateways)]
	pub(super) type Gateways<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		MassbitId,
		Gateway<AccountIdOf<T>, BalanceOf<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn nodes)]
	pub(super) type Nodes<T: Config> =
		StorageMap<_, Blake2_128Concat, MassbitId, Node<AccountIdOf<T>, BalanceOf<T>>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn register_project(
			origin: OriginFor<T>,
			project_id: MassbitId,
			blockchain: BlockChain,
			deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(<Gateways<T>>::get(&project_id).is_none(), Error::<T>::AlreadyRegistered);

			let quota = Self::calculate_consumer_quota(deposit);
			<Projects<T>>::insert(
				&project_id,
				Project { owner: account.clone(), blockchain: blockchain.clone(), quota, usage: 0 },
			);

			Self::deposit_event(Event::ProjectRegistered(project_id, account, blockchain, quota));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn register_gateway(
			origin: OriginFor<T>,
			gateway_id: MassbitId,
			blockchain: BlockChain,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(<Gateways<T>>::get(&gateway_id).is_none(), Error::<T>::AlreadyRegistered);

			ensure!(deposit >= T::MinGatewayDeposit::get(), Error::<T>::InsufficientBoding);

			T::Staking::bond_and_stake(
				account.clone(),
				T::Hashing::hash_of(&blockchain),
				deposit.clone(),
			)?;

			<Gateways<T>>::insert(
				&gateway_id,
				Gateway { owner: account.clone(), blockchain: blockchain.clone(), deposit },
			);

			Self::deposit_event(Event::GatewayRegistered(gateway_id, account, blockchain, deposit));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn register_node(
			origin: OriginFor<T>,
			node_id: MassbitId,
			deposit: BalanceOf<T>,
			blockchain: BlockChain,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(<Nodes<T>>::get(&node_id).is_none(), Error::<T>::AlreadyRegistered);

			ensure!(deposit >= T::MinNodeDeposit::get(), Error::<T>::InsufficientBoding);

			T::Staking::bond_and_stake(
				account.clone(),
				T::Hashing::hash_of(&blockchain),
				deposit.clone(),
			)?;

			<Nodes<T>>::insert(
				&node_id,
				Node { owner: account.clone(), blockchain: blockchain.clone(), deposit },
			);

			Self::deposit_event(Event::NodeRegistered(node_id, account, blockchain, deposit));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_project_usage(
			origin: OriginFor<T>,
			project_id: MassbitId,
			usage: u64,
		) -> DispatchResultWithPostInfo {
			let oracle = ensure_signed(origin)?;

			ensure!(T::IsOracle::is_member(&oracle), Error::<T>::NotOracle);

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::ProjectNotFound)?;
			project.usage.saturating_add(usage);
			Projects::<T>::insert(&project_id, project);

			Self::deposit_event(Event::ProjectUsageReported(project_id, usage));

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		fn calculate_consumer_quota(amount: BalanceOf<T>) -> u64 {
			TryInto::<u64>::try_into(amount).ok().unwrap_or_default()
		}
	}
}
