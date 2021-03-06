use crate::{self as pallet_dapi, weights};

use frame_support::{
	construct_runtime, parameter_types,
	traits::{Currency, OnFinalize, OnInitialize, OnUnbalanced},
	PalletId,
};
use sp_core::{H160, H256};

use codec::{Decode, Encode};
use frame_support::traits::ConstU32;
use frame_system::EnsureRoot;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

pub(crate) type AccountId = u64;
pub(crate) type BlockNumber = u64;
pub(crate) type Balance = u128;
pub(crate) type EraIndex = u32;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

/// Value shouldn't be less than 2 for testing purposes, otherwise we cannot test certain corner
/// cases.
pub(crate) const EXISTENTIAL_DEPOSIT: Balance = 2;
pub(crate) const MAX_NUMBER_OF_STAKERS: u32 = 5;
/// Value shouldn't be less than 2 for testing purposes, otherwise we cannot test certain corner
/// cases.
pub(crate) const MINIMUM_STAKING_AMOUNT: Balance = 10;
pub(crate) const OPERATOR_REWARD_PERCENTAGE: u32 = 80;
pub(crate) const MINIMUM_REMAINING_AMOUNT: Balance = 1;
pub(crate) const MAX_UNLOCKING_CHUNKS: u32 = 4;
pub(crate) const UNBONDING_PERIOD: EraIndex = 3;
pub(crate) const MAX_ERA_STAKE_VALUES: u32 = 8;

// Do note that this needs to at least be 3 for tests to be valid. It can be greater but not
// smaller.
pub(crate) const BLOCKS_PER_ERA: BlockNumber = 3;

pub(crate) const REGISTER_DEPOSIT: Balance = 10;

// ignore MILLIMBT for easier test handling.
// reward for dapi staking will be BLOCK_REWARD/2 = 1000
pub(crate) const BLOCK_REWARD: Balance = 1000;

construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		DapiStaking: pallet_dapi_staking::{Pallet, Call, Storage, Event<T>},
		Dapi: pallet_dapi::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1024);
}

impl frame_system::Config for TestRuntime {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Index = u64;
	type Call = Call;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const MaxLocks: u32 = 4;
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for TestRuntime {
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 3;
}

impl pallet_timestamp::Config for TestRuntime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const RegisterDeposit: Balance = REGISTER_DEPOSIT;
	pub const BlockPerEra: BlockNumber = BLOCKS_PER_ERA;
	pub const MaxNumberOfStakersPerProvider: u32 = MAX_NUMBER_OF_STAKERS;
	pub const MinimumStakingAmount: Balance = MINIMUM_STAKING_AMOUNT;
	pub const OperatorRewardPercentage: Perbill = Perbill::from_percent(OPERATOR_REWARD_PERCENTAGE);
	pub const DapiStakingPalletId: PalletId = PalletId(*b"mokdpstk");
	pub const MinimumRemainingAmount: Balance = MINIMUM_REMAINING_AMOUNT;
	pub const MaxUnlockingChunks: u32 = MAX_UNLOCKING_CHUNKS;
	pub const UnbondingPeriod: EraIndex = UNBONDING_PERIOD;
	pub const MaxEraStakeValues: u32 = MAX_ERA_STAKE_VALUES;
}

impl pallet_dapi_staking::Config for TestRuntime {
	type Event = Event;
	type Currency = Balances;
	type BlockPerEra = BlockPerEra;
	type RegisterDeposit = RegisterDeposit;
	type OperatorRewardPercentage = OperatorRewardPercentage;
	type ProviderId = MockProvider;
	type MaxNumberOfStakersPerProvider = MaxNumberOfStakersPerProvider;
	type MinimumStakingAmount = MinimumStakingAmount;
	type PalletId = DapiStakingPalletId;
	type MinimumRemainingAmount = MinimumRemainingAmount;
	type MaxUnlockingChunks = MaxUnlockingChunks;
	type UnbondingPeriod = UnbondingPeriod;
	type MaxEraStakeValues = MaxEraStakeValues;
	type WeightInfo = pallet_dapi_staking::weights::SubstrateWeight<TestRuntime>;
}

#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, Debug, scale_info::TypeInfo)]
pub struct MockProvider([u8; 36]);

impl Default for MockProvider {
	fn default() -> Self {
		MockProvider([1; 36])
	}
}

parameter_types! {
	pub const ProjectDepositPeriod: BlockNumber = 10;
}

impl pallet_dapi::Config for TestRuntime {
	type Event = Event;
	type Currency = Balances;
	type DapiStaking = DapiStaking;
	type UpdateRegulatorOrigin = EnsureRoot<AccountId>;
	type ChainIdMaxLength = ConstU32<64>;
	type MassbitId = MockProvider;
	type OnProjectPayment = ();
	type WeightInfo = weights::SubstrateWeight<TestRuntime>;
}

pub struct ExternalityBuilder;

impl ExternalityBuilder {
	pub fn build() -> TestExternalities {
		let mut storage =
			frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

		pallet_balances::GenesisConfig::<TestRuntime> {
			balances: vec![
				(1, 9000),
				(2, 800),
				(3, 10000),
				(4, 4900),
				(5, 3800),
				(6, 10),
				(7, 1000),
				(8, 2000),
				(9, 10000),
				(10, 300),
				(11, 400),
				(20, 10),
				(540, EXISTENTIAL_DEPOSIT),
				(1337, 1_000_000_000_000),
			],
		}
		.assimilate_storage(&mut storage)
		.ok();

		let mut ext = TestExternalities::from(storage);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
