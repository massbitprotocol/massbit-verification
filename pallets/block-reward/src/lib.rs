#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, OnTimestampSet, OnUnbalanced},
	};

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Currency: Currency<Self::AccountId>;

		/// Handle block reward as imbalance.
		type OnBlockReward: OnUnbalanced<
			<Self::Currency as Currency<Self::AccountId>>::NegativeImbalance,
		>;

		/// The amount of issuance for each block.
		#[pallet::constant]
		type RewardAmount: Get<BalanceOf<Self>>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	impl<Moment, T: Config> OnTimestampSet<Moment> for Pallet<T> {
		fn on_timestamp_set(_: Moment) {
			let inflation = T::Currency::issue(T::RewardAmount::get());
			T::OnBlockReward::on_unbalanced(inflation);
		}
	}
}
