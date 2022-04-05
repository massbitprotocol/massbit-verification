#![cfg_attr(not(feature = "std"), no_std)]

use super::*;
use frame_support::pallet_prelude::*;
use sp_runtime::traits::AtLeast32BitUnsigned;

#[derive(Clone, Copy, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct Deposit<Balance, BlockNumber> {
	/// Amount being deposited.
	#[codec(compact)]
	pub amount: Balance,
	/// Block number in which the amount will become unreserved.
	#[codec(compact)]
	pub unreserved_block_number: BlockNumber,
}

/// Contains deposit chunks.
#[derive(Clone, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct DepositInfo<
	Balance: AtLeast32BitUnsigned + Default + Copy,
	BlockNumber: AtLeast32BitUnsigned + Default + Copy,
> {
	deposit_chunks: Vec<Deposit<Balance, BlockNumber>>,
}

impl<Balance, BlockNumber> DepositInfo<Balance, BlockNumber>
where
	Balance: AtLeast32BitUnsigned + Default + Copy,
	BlockNumber: AtLeast32BitUnsigned + Default + Copy,
{
	/// Returns total number of deposit chunks.
	pub fn len(&self) -> u32 {
		self.deposit_chunks.len() as u32
	}

	/// True if no deposit chunks exist, false otherwise.
	pub fn is_empty(&self) -> bool {
		self.deposit_chunks.is_empty()
	}

	/// Returns sum of all deposit chunks.
	pub fn sum(&self) -> Balance {
		self.deposit_chunks
			.iter()
			.map(|chunk| chunk.amount)
			.reduce(|c1, c2| c1 + c2)
			.unwrap_or_default()
	}

	/// Adds a new deposit chunk to the vector.
	pub fn add(&mut self, chunk: Deposit<Balance, BlockNumber>) {
		self.deposit_chunks.push(chunk);
	}

	/// Partitions the deposit chunks into two groups:
	///
	/// First group includes all chunks which have unreserved block number lesser or equal to the
	/// specified block number. Second group includes all the rest.
	///
	/// Order of chunks is preserved in the two new structs.
	pub fn partition(self, block_number: BlockNumber) -> (Self, Self) {
		let (matching_chunks, other_chunks): (
			Vec<Deposit<Balance, BlockNumber>>,
			Vec<Deposit<Balance, BlockNumber>>,
		) = self
			.deposit_chunks
			.iter()
			.partition(|chunk| chunk.unreserved_block_number <= block_number);

		(Self { deposit_chunks: matching_chunks }, Self { deposit_chunks: other_chunks })
	}
}

#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct Project<
	AccountId,
	ChainId,
	Balance: AtLeast32BitUnsigned + Default + Copy,
	BlockNumber: AtLeast32BitUnsigned + Default + Copy,
> {
	pub consumer: AccountId,
	pub chain_id: ChainId,
	pub quota: u128,
	pub usage: u128,
	pub deposit_info: DepositInfo<Balance, BlockNumber>,
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
