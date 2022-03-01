#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_support::{
		sp_runtime::traits::Hash,
		traits::{Currency, LockIdentifier, LockableCurrency, Randomness},
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_io::hashing::blake2_128;
	use sp_std::{convert::TryInto, prelude::*};

	#[cfg(feature = "std")]
	use frame_support::serde::{Deserialize, Serialize};
	use frame_support::traits::WithdrawReasons;

	const LOCK_IDENT: LockIdentifier = *b"dapi    ";

	type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum BlockChain {
		Ethereum,
		Polkadot,
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Consumer<AccountId> {
		pub owner: AccountId,
		pub blockchain: BlockChain,
		pub quota: i64,
	}

	type ConsumerOf<T> = Consumer<AccountIdOf<T>>;

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Gateway<AccountId, Balance> {
		pub owner: AccountId,
		pub blockchain: BlockChain,
		pub deposit: Balance,
	}

	type GatewayOf<T> = Gateway<AccountIdOf<T>, BalanceOf<T>>;

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Node<AccountId, Balance> {
		pub owner: AccountId,
		pub blockchain: BlockChain,
		pub deposit: Balance,
	}

	type NodeOf<T> = Node<AccountIdOf<T>, BalanceOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

		type MinConsumerDeposit: Get<BalanceOf<Self>>;

		type MinGatewayDeposit: Get<BalanceOf<Self>>;

		type MinNodeDeposit: Get<BalanceOf<Self>>;

		type IdRandomness: Randomness<Self::Hash, Self::BlockNumber>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::error]
	pub enum Error<T> {
		ConsumerDepositNotEnough,
		GatewayDepositNotEnough,
		NodeDepositNotEnough,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A consumer is successfully created. \[account_id, consumer_id, blockchain_type\]
		ConsumerCreated(T::AccountId, T::Hash, BlockChain),
		/// A gateway is successfully created. \[account_id, gateway_id, blockchain_type\]
		GatewayCreated(T::AccountId, T::Hash, BlockChain),
		/// A node is successfully created. \[account_id, node_id, blockchain_type\]
		NodeCreated(T::AccountId, T::Hash, BlockChain),
	}

	#[pallet::storage]
	#[pallet::getter(fn consumers)]
	pub(super) type Consumers<T: Config> =
		StorageMap<_, Twox64Concat, T::Hash, ConsumerOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn gateways)]
	pub(super) type Gateways<T: Config> =
		StorageMap<_, Twox64Concat, T::Hash, GatewayOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn nodes)]
	pub(super) type Nodes<T: Config> = StorageMap<_, Twox64Concat, T::Hash, NodeOf<T>, OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn create_consumer(
			origin: OriginFor<T>,
			deposit: BalanceOf<T>,
			blockchain: BlockChain,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(deposit >= T::MinConsumerDeposit::get(), Error::<T>::ConsumerDepositNotEnough);
			T::Currency::set_lock(LOCK_IDENT, &account, deposit, WithdrawReasons::all());

			let consumer = Consumer {
				owner: account.clone(),
				blockchain: blockchain.clone(),
				quota: Self::calculate_consumer_quota(deposit),
			};
			let consumer_id = T::Hashing::hash_of(&Self::gen_id());
			<Consumers<T>>::insert(consumer_id, consumer);

			Self::deposit_event(Event::ConsumerCreated(account, consumer_id, blockchain));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn create_gateway(
			origin: OriginFor<T>,
			deposit: BalanceOf<T>,
			blockchain: BlockChain,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(deposit >= T::MinGatewayDeposit::get(), Error::<T>::GatewayDepositNotEnough);
			T::Currency::set_lock(LOCK_IDENT, &account, deposit, WithdrawReasons::all());

			let gateway =
				Gateway { owner: account.clone(), blockchain: blockchain.clone(), deposit };
			let gateway_id = T::Hashing::hash_of(&Self::gen_id());
			<Gateways<T>>::insert(gateway_id, gateway);

			Self::deposit_event(Event::GatewayCreated(account, gateway_id, blockchain));

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn create_node(
			origin: OriginFor<T>,
			deposit: BalanceOf<T>,
			blockchain: BlockChain,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			ensure!(deposit >= T::MinNodeDeposit::get(), Error::<T>::NodeDepositNotEnough);
			T::Currency::set_lock(LOCK_IDENT, &account, deposit, WithdrawReasons::all());

			let node = Node { owner: account.clone(), blockchain: blockchain.clone(), deposit };
			let node_id = T::Hashing::hash_of(&Self::gen_id());
			<Nodes<T>>::insert(node_id, node);

			Self::deposit_event(Event::GatewayCreated(account, node_id, blockchain));

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		fn gen_id() -> [u8; 16] {
			let payload = (
				T::IdRandomness::random(&b"id"[..]).0,
				<frame_system::Pallet<T>>::extrinsic_index().unwrap_or_default(),
				<frame_system::Pallet<T>>::block_number(),
			);
			payload.using_encoded(blake2_128)
		}

		fn calculate_consumer_quota(amount: BalanceOf<T>) -> i64 {
			TryInto::<u64>::try_into(amount).ok().unwrap_or_default() as i64
		}
	}
}
