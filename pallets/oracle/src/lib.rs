#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::traits::IsMember;
use sp_std::{collections::btree_set::BTreeSet, iter::FromIterator, prelude::*};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The origin which can add an oracle.
		type AddOrigin: EnsureOrigin<Self::Origin>;
	}

	/// The set of oracles.
	#[pallet::storage]
	#[pallet::getter(fn oracles)]
	pub type Oracles<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub oracles: Vec<T::AccountId>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { oracles: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_oracles(&self.oracles);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}
}

impl<T: Config> Pallet<T> {
	fn initialize_oracles(oracles: &Vec<T::AccountId>) {
		let oracle_ids =
			oracles.iter().map(|oracle| oracle.clone()).collect::<BTreeSet<T::AccountId>>();
		Oracles::<T>::put(&oracle_ids);
	}
}

impl<T: Config> IsMember<T::AccountId> for Pallet<T> {
	fn is_member(oracle_id: &T::AccountId) -> bool {
		Self::oracles().iter().any(|id| id == oracle_id)
	}
}
