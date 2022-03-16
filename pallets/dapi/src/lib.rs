#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use frame_support::serde::{Deserialize, Serialize};
use frame_support::{
	sp_runtime::traits::Hash,
	traits::{Currency, LockableCurrency},
};
use scale_info::TypeInfo;
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

		type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

		type MinConsumerDeposit: Get<BalanceOf<Self>>;

		type MinGatewayDeposit: Get<BalanceOf<Self>>;

		type MinNodeDeposit: Get<BalanceOf<Self>>;

		type Staking: Staking<BalanceOf<Self>, Self::AccountId, Self::Hash>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::error]
	pub enum Error<T> {
		AlreadyRegistered,
		InsufficientBoding,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A project is successfully registered. \[project_id, project_hash, account_id,
		/// blockchain, quota\]
		ProjectRegistered(Vec<u8>, T::Hash, T::AccountId, BlockChain, u64),
		/// A gateway is successfully registered. \[gateway_id, gateway_hash, account_id,
		/// blockchain\]
		GatewayRegistered(Vec<u8>, T::Hash, T::AccountId, BlockChain),
		/// A node is successfully registered. \[node_id, node_hash, account_id, blockchain\]
		NodeRegistered(Vec<u8>, T::Hash, T::AccountId, BlockChain),
	}

	#[pallet::storage]
	#[pallet::getter(fn consumers)]
	pub(super) type Consumers<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, Project<AccountIdOf<T>>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn gateways)]
	pub(super) type Gateways<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Gateway<AccountIdOf<T>, BalanceOf<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn nodes)]
	pub(super) type Nodes<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, Node<AccountIdOf<T>, BalanceOf<T>>, OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn register_project(
			origin: OriginFor<T>,
			project_id: Vec<u8>,
			blockchain: BlockChain,
			deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			let project_hash = T::Hashing::hash_of(&project_id);
			ensure!(<Gateways<T>>::get(project_hash).is_none(), Error::<T>::AlreadyRegistered);

			let quota = Self::calculate_consumer_quota(deposit);
			<Consumers<T>>::insert(
				project_hash,
				Project { owner: account.clone(), blockchain: blockchain.clone(), quota },
			);

			Self::deposit_event(Event::ProjectRegistered(
				project_id,
				project_hash,
				account,
				blockchain,
				quota,
			));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn register_gateway(
			origin: OriginFor<T>,
			gateway_id: Vec<u8>,
			blockchain: BlockChain,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			let gateway_hash = T::Hashing::hash_of(&gateway_id);
			ensure!(<Gateways<T>>::get(gateway_hash).is_none(), Error::<T>::AlreadyRegistered);

			ensure!(deposit >= T::MinGatewayDeposit::get(), Error::<T>::InsufficientBoding);

			T::Staking::bond_and_stake(
				account.clone(),
				T::Hashing::hash_of(&blockchain),
				deposit.clone(),
			)?;

			<Gateways<T>>::insert(
				gateway_hash,
				Gateway { owner: account.clone(), blockchain: blockchain.clone(), deposit },
			);

			Self::deposit_event(Event::GatewayRegistered(
				gateway_id,
				gateway_hash,
				account,
				blockchain,
			));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn register_node(
			origin: OriginFor<T>,
			node_id: Vec<u8>,
			deposit: BalanceOf<T>,
			blockchain: BlockChain,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			let node_hash = T::Hashing::hash_of(&node_id);
			ensure!(<Nodes<T>>::get(node_hash).is_none(), Error::<T>::AlreadyRegistered);

			ensure!(deposit >= T::MinNodeDeposit::get(), Error::<T>::InsufficientBoding);

			T::Staking::bond_and_stake(
				account.clone(),
				T::Hashing::hash_of(&blockchain),
				deposit.clone(),
			)?;

			<Nodes<T>>::insert(
				node_hash,
				Node { owner: account.clone(), blockchain: blockchain.clone(), deposit },
			);

			Self::deposit_event(Event::NodeRegistered(node_id, node_hash, account, blockchain));

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		fn calculate_consumer_quota(amount: BalanceOf<T>) -> u64 {
			TryInto::<u64>::try_into(amount).ok().unwrap_or_default()
		}
	}
}
