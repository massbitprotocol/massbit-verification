//! # Block Reward Pallet
//!
//! - [`Config`]
//!
//! ## Overview
//!
//! Simple pallet that implements block reward mechanics.
//!
//! ## Interface
//!
//! This pallet implements the `OnTimestampSet` trait to handle block production.
//! Note: We assume that it's impossible to set timestamp two times in a block.
//!
//! ## Usage
//!
//! 1. Pallet should be set as a handler of `OnTimestampSet`.
//! 2. `OnBlockReward` handler should be defined as an implementation of `OnUnbalanced` trait. For
//! example:
//! ```nocompile
//! type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;
//! struct SaveOnDapiStaking;
//! impl OnUnbalanced<NegativeImbalance> for SaveOnDapiStaking {
//!   fn on_nonzero_unbalanced(amount: NegativeImbalance) {
//!     Balances::resolve_creating(&DapiStaking::pallet_id(), amount);
//!   }
//! }
//! ```
//! 3. Set `RewardAmount` to desired block reward value in native currency.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{Currency, OnTimestampSet, OnUnbalanced};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The pallet currency type.
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
