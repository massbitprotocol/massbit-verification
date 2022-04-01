use super::{pallet::pallet::Error, Event, *};
use frame_support::{
    assert_noop, assert_ok,
    traits::{OnInitialize, OnUnbalanced},
};
use mock::{Balances, MockProvider, *};
use sp_core::H160;
use sp_runtime::{
    traits::{BadOrigin, Zero},
    Perbill,
};

use testing_utils::*;

#[test]
fn on_initialize_when_dapp_staking_enabled_in_mid_of_an_era_is_ok() {
    ExternalityBuilder::build().execute_with(|| {
        // Set a block number in mid of an era
        System::set_block_number(2);

        // Verify that current era is 0 since dapi staking hasn't been initialized yet
        assert_eq!(0u32, DapiStaking::current_era());

        // Call on initialize in the mid of an era (according to block number calculation)
        // but since no era was initialized before, it will trigger a new era init.
        DapiStaking::on_initialize(System::block_number());
        assert_eq!(1u32, DapiStaking::current_era());
    })
}

#[test]
fn on_unbalanced_is_ok() {
    ExternalityBuilder::build().execute_with(|| {
        // At the beginning, both should be 0
        assert_eq!(
            BlockRewardAccumulator::<TestRuntime>::get(),
            Default::default()
        );
        assert!(free_balance_of_dapps_staking_account().is_zero());

        // After handling imbalance, accumulator and account should be updated
        DapiStaking::on_unbalanced(Balances::issue(BLOCK_REWARD));
        let block_reward = BlockRewardAccumulator::<TestRuntime>::get();
        assert_eq!(BLOCK_REWARD, block_reward.stakers + block_reward.operators);

        let expected_operators_reward =
            <TestRuntime as Config>::OperatorRewardPercentage::get() * BLOCK_REWARD;
        let expected_stakers_reward = BLOCK_REWARD - expected_operators_reward;
        assert_eq!(block_reward.stakers, expected_stakers_reward);
        assert_eq!(block_reward.operators, expected_operators_reward);

        assert_eq!(BLOCK_REWARD, free_balance_of_dapps_staking_account());

        // After triggering a new era, accumulator should be set to 0 but account shouldn't consume any new imbalance
        DapiStaking::on_initialize(System::block_number());
        assert_eq!(
            BlockRewardAccumulator::<TestRuntime>::get(),
            Default::default()
        );
        assert_eq!(BLOCK_REWARD, free_balance_of_dapps_staking_account());
    })
}

#[test]
fn on_initialize_is_ok() {
    ExternalityBuilder::build().execute_with(|| {
        // Before we start, era is zero
        assert!(DapiStaking::current_era().is_zero());

        // We initialize the first block and advance to second one. New era must be triggered.
        initialize_first_block();
        let current_era = DapiStaking::current_era();
        assert_eq!(1, current_era);

        let previous_era = current_era;
        advance_to_era(previous_era + 10);

        // Check that all reward&stakes are as expected
        let current_era = DapiStaking::current_era();
        for era in 1..current_era {
            let reward_info = GeneralEraInfo::<TestRuntime>::get(era).unwrap().rewards;
            assert_eq!(
                get_total_reward_per_era(),
                reward_info.stakers + reward_info.operators
            );
        }
        // Current era rewards should be 0
        let era_rewards = GeneralEraInfo::<TestRuntime>::get(current_era).unwrap();
        assert_eq!(0, era_rewards.staked);
        assert_eq!(era_rewards.rewards, Default::default());
    })
}

#[test]
fn new_era_length_is_always_blocks_per_era() {
    ExternalityBuilder::build().execute_with(|| {
        initialize_first_block();
        let blocks_per_era = mock::BLOCKS_PER_ERA;

        // go to beginning of an era
        advance_to_era(mock::DapiStaking::current_era() + 1);

        // record era number and block number
        let start_era = mock::DapiStaking::current_era();
        let starting_block_number = System::block_number();

        // go to next era
        advance_to_era(mock::DapiStaking::current_era() + 1);
        let ending_block_number = System::block_number();

        // make sure block number difference is is blocks_per_era
        assert_eq!(mock::DapiStaking::current_era(), start_era + 1);
        assert_eq!(ending_block_number - starting_block_number, blocks_per_era);
    })
}

// #[test]
// fn new_era_is_handled_with_maintenance_mode() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         // enable maintenance mode
//         assert_ok!(DapiStaking::maintenance_mode(Origin::root(), true));
//         assert!(PalletDisabled::<TestRuntime>::exists());
//         System::assert_last_event(mock::Event::DapiStaking(Event::MaintenanceMode(true)));
//
//         // advance 9 blocks or 3 era lengths (advance_to_era() doesn't work in maintenance mode)
//         run_for_blocks(mock::BLOCKS_PER_ERA * 3);
//
//         // verify that `current block > NextEraStartingBlock` but era hasn't changed
//         assert!(System::block_number() > DapiStaking::next_era_starting_block());
//         assert_eq!(DapiStaking::current_era(), 1);
//
//         // disable maintenance mode
//         assert_ok!(DapiStaking::maintenance_mode(Origin::root(), false));
//         System::assert_last_event(mock::Event::DapiStaking(Event::MaintenanceMode(false)));
//
//         // advance one era
//         run_for_blocks(mock::BLOCKS_PER_ERA);
//
//         // verify we're at block 14
//         assert_eq!(System::block_number(), (4 * mock::BLOCKS_PER_ERA) + 2); // 2 from initialization, advanced 4 eras worth of blocks
//
//         // verify era was updated and NextEraStartingBlock is 15
//         assert_eq!(DapiStaking::current_era(), 2);
//         assert_eq!(
//             DapiStaking::next_era_starting_block(),
//             (5 * mock::BLOCKS_PER_ERA)
//         );
//     })
// }

// #[test]
// fn new_forced_era_length_is_always_blocks_per_era() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//         let blocks_per_era = mock::BLOCKS_PER_ERA;
//
//         // go to beginning of an era
//         advance_to_era(mock::DapiStaking::current_era() + 1);
//
//         // go to middle of era
//         run_for_blocks(1); // can be any number between 0 and blocks_per_era
//
//         // force new era
//         <ForceEra<TestRuntime>>::put(Forcing::ForceNew);
//         run_for_blocks(1); // calls on_initialize()
//
//         // note the start block number of new (forced) era
//         let start_block_number = System::block_number();
//
//         // go to start of next era
//         advance_to_era(mock::DapiStaking::current_era() + 1);
//
//         // show the length of the forced era is equal to blocks_per_era
//         let end_block_number = System::block_number();
//         assert_eq!(end_block_number - start_block_number, blocks_per_era);
//     })
// }

#[test]
fn new_era_is_ok() {
    ExternalityBuilder::build().execute_with(|| {
        // set initial era index
        advance_to_era(DapiStaking::current_era() + 10);
        let starting_era = DapiStaking::current_era();

        // verify that block reward is zero at the beginning of an era
        assert_eq!(DapiStaking::block_reward_accumulator(), Default::default());

        // Increment block by setting it to the first block in era value
        run_for_blocks(1);
        let current_era = DapiStaking::current_era();
        assert_eq!(starting_era, current_era);

        // verify that block reward is added to the block_reward_accumulator
        let block_reward = DapiStaking::block_reward_accumulator();
        assert_eq!(BLOCK_REWARD, block_reward.stakers + block_reward.operators);

        // register to verify storage item
        let staker = 2;
        let provider_acc = 3;
        let staked_amount = 100;
        let deposit = 200;
        //let provider = MockProvider(*b"00000000-0000-0000-0000-000000000001");
        let provider = MockProvider::default();
        assert_register_provider(provider_acc, &provider,deposit);
        assert_stake(staker, &provider, staked_amount);

        // CurrentEra should be incremented
        // block_reward_accumulator should be reset to 0
        advance_to_era(DapiStaking::current_era() + 1);

        let current_era = DapiStaking::current_era();
        assert_eq!(starting_era + 1, current_era);
        System::assert_last_event(mock::Event::DapiStaking(Event::NewDapiStakingEra(
            starting_era + 1,
        )));

        // verify that block reward accumulator is reset to 0
        let block_reward = DapiStaking::block_reward_accumulator();
        assert_eq!(block_reward, Default::default());

        let expected_era_reward = get_total_reward_per_era();
        let expected_operators_reward =
            <TestRuntime as Config>::OperatorRewardPercentage::get() * expected_era_reward;
        let expected_stakers_reward = expected_era_reward - expected_operators_reward;

        // verify that .staked is copied and .reward is added
        let era_rewards = GeneralEraInfo::<TestRuntime>::get(starting_era).unwrap();
        assert_eq!(staked_amount+deposit, era_rewards.staked+RegisterDeposit::get());
        assert_eq!(
            expected_era_reward,
            era_rewards.rewards.operators + era_rewards.rewards.stakers
        );
        assert_eq!(expected_operators_reward, era_rewards.rewards.operators);
        assert_eq!(expected_stakers_reward, era_rewards.rewards.stakers);
    })
}

#[test]
fn new_era_forcing() {
    ExternalityBuilder::build().execute_with(|| {
        initialize_first_block();
        advance_to_era(3);
        let starting_era = mock::DapiStaking::current_era();

        // call on_initilize. It is not last block in the era, but it should increment the era
        <ForceEra<TestRuntime>>::put(Forcing::ForceNew);
        run_for_blocks(1);

        // check that era is incremented
        let current = mock::DapiStaking::current_era();
        assert_eq!(starting_era + 1, current);

        // check that forcing is cleared
        assert_eq!(mock::DapiStaking::force_era(), Forcing::NotForcing);

        // check the event for the new era
        System::assert_last_event(mock::Event::DapiStaking(Event::NewDapiStakingEra(
            starting_era + 1,
        )));
    })
}

#[test]
fn general_staker_info_is_ok() {
    ExternalityBuilder::build().execute_with(|| {
        initialize_first_block();

        let deposit = 200;

        let first_provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
        assert_register_provider(10, &first_provider_id, deposit);

        let (staker_1, staker_2, staker_3) = (1, 2, 3);
        let amount = 100;

        let starting_era = 3;
        advance_to_era(starting_era);
        assert_stake(staker_1, &first_provider_id, amount);
        assert_stake(staker_2, &first_provider_id, amount);


        let mid_era = 7;
        advance_to_era(mid_era);

        let second_provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000002");
        assert_register_provider(11, &second_provider_id,deposit);


        assert_unstake(staker_2, &first_provider_id, amount);
        assert_stake(staker_3, &first_provider_id, amount);
        assert_stake(staker_3, &second_provider_id, amount);

        let final_era = 12;
        advance_to_era(final_era);

        // Check first interval
        let mut first_staker_info = DapiStaking::staker_info(&staker_1, &first_provider_id);
        let mut second_staker_info = DapiStaking::staker_info(&staker_2, &first_provider_id);
        let mut third_staker_info = DapiStaking::staker_info(&staker_3, &first_provider_id);

        for era in starting_era..mid_era {
            let provider_info = DapiStaking::provider_stake_info(&first_provider_id, era).unwrap();
            //println!("provider_info:{:?}",provider_info);
            assert_eq!(3, provider_info.number_of_stakers);
            assert_eq!((era, amount), first_staker_info.claim());
            assert_eq!((era, amount), second_staker_info.claim());

            assert!(!ProviderEraStake::<TestRuntime>::contains_key(
                &second_provider_id,
                era
            ));
        }

        // Check second interval
        for era in mid_era..=final_era {
            let first_provider_info =
                DapiStaking::provider_stake_info(&first_provider_id, era).unwrap();
            assert_eq!(3, first_provider_info.number_of_stakers);

            assert_eq!((era, amount), first_staker_info.claim());
            assert_eq!((era, amount), third_staker_info.claim());

            assert_eq!(
                DapiStaking::provider_stake_info(&second_provider_id, era)
                    .unwrap()
                    .number_of_stakers,
                2
            );
        }

        // Check that before starting era only 1 first_provider_id staking
        // assert!(!ProviderEraStake::<TestRuntime>::contains_key(
        //     &first_provider_id,
        //     starting_era - 1
        // ));
        assert!(!ProviderEraStake::<TestRuntime>::contains_key(
            &second_provider_id,
            starting_era - 1
        ));
    })
}

#[test]
fn register_is_ok() {
    ExternalityBuilder::build().execute_with(|| {
        initialize_first_block();

        let operator = 1;
        let ok_provider = MockProvider(*b"00000000-0000-0000-0000-000000000001");
        let deposit = 200;

        assert!(<TestRuntime as Config>::Currency::reserved_balance(&operator).is_zero());
        assert_register_provider(operator, &ok_provider,deposit);
        System::assert_last_event(mock::Event::DapiStaking(Event::Stake(
            operator,
            ok_provider,
            deposit-RegisterDeposit::get(),
        )));

        assert_eq!(
            RegisterDeposit::get(),
            <TestRuntime as Config>::Currency::reserved_balance(&operator)
        );
    })
}

// #[test]
// fn register_twice_with_same_account_fails() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let provider1 = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         let provider2 = MockProvider(*b"00000000-0000-0000-0000-000000000002");
//         let deposit = 200;
//
//         assert_register_provider(operator, &provider1, deposit);
//
//         System::assert_last_event(mock::Event::DapiStaking(Event::Stake(
//             operator, provider1, deposit-RegisterDeposit::get()
//         )));
//
//         // now register different provider with same account
//         // Fixme: Error should be AlreadyRegisteredOperator or it is ok for use the same Operator for 2 provider
//         assert_noop!(
//             DapiStaking::register(operator, provider2,deposit),
//             Error::<TestRuntime>::AlreadyRegisteredProvider
//         );
//     })
// }

#[test]
fn register_same_provider_twice_fails() {
    ExternalityBuilder::build().execute_with(|| {
        initialize_first_block();

        let operator1 = 1;
        let operator2 = 2;
        let provider = MockProvider(*b"00000000-0000-0000-0000-000000000001");
        let deposit = 200;
        assert_register_provider(operator1, &provider,deposit);

        System::assert_last_event(mock::Event::DapiStaking(Event::Stake(
            operator1, provider,deposit-RegisterDeposit::get()
        )));

        // now register same provider by different operator
        assert_noop!(
            DapiStaking::register(operator2, provider,deposit),
            Error::<TestRuntime>::AlreadyRegisteredProvider
        );
    })
}

// #[test]
// fn register_with_pre_approve_enabled() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//         let operator = 1;
//         let provider = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         // enable pre-approval for the operators
//         assert_ok!(DapiStaking::enable_operator_pre_approval(
//             Origin::root(),
//             true
//         ));
//         assert!(DapiStaking::pre_approval_is_enabled());
//
//         // register new operator without pre-approval, should fail
//         assert_noop!(
//             DapiStaking::register(Origin::signed(operator), provider.clone()),
//             Error::<TestRuntime>::RequiredContractPreApproval,
//         );
//
//         // preapprove operator
//         assert_ok!(DapiStaking::operator_pre_approval(
//             Origin::root(),
//             operator.clone()
//         ));
//
//         // try to pre-approve again same operator, should fail
//         assert_noop!(
//             DapiStaking::operator_pre_approval(Origin::root(), operator.clone()),
//             Error::<TestRuntime>::AlreadyPreApprovedDeveloper
//         );
//
//         // register new provider by pre-approved operator
//         assert_register_provider(operator, &provider);
//
//         // disable pre_approval and register provider2
//         assert_ok!(DapiStaking::enable_operator_pre_approval(
//             Origin::root(),
//             false
//         ));
//
//         let operator2 = 2;
//         let provider2 = MockProvider(*b"00000000-0000-0000-0000-000000000002");
//         assert_register_provider(operator2, &provider2);
//     })
// }

#[test]
fn unregister_after_register_is_ok() {
    ExternalityBuilder::build().execute_with(|| {
        initialize_first_block();

        let operator = 1;
        let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");

        let deposit = 200;

        let starting_era = 3;
        advance_to_era(starting_era);

        assert_register_provider(operator, &provider_id,deposit);
        assert_unregister(operator, &provider_id);

        let unbonding_era = UnbondingPeriod::get();
        advance_to_era(starting_era + unbonding_era);
        //assert_eq!((starting_era + unbonding_era, amount), operator.claim());

        assert!(!ProviderEraStake::<TestRuntime>::contains_key(
            &provider_id,
            DapiStaking::current_era()
        ));
        // Fixme: add check claim
        //assert!(<TestRuntime as Config>::Currency::reserved_balance(&operator).is_zero());

        // Not possible to unregister a provider twice
        assert_noop!(
            DapiStaking::unregister(provider_id.clone()),
            Error::<TestRuntime>::NotOperatedProvider
        );
    })
}

// #[test]
// fn unregister_with_non_root() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
// 
//         let operator = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
// 
//         assert_register_provider(operator, &provider_id,200);
// 
//         // Not possible to unregister if caller isn't root
//         assert_noop!(
//             DapiStaking::unregister(provider_id.clone()),
//             BadOrigin
//         );
//     })
// }

#[test]
fn unregister_stake_and_unstake_is_not_ok() {
    ExternalityBuilder::build().execute_with(|| {
        initialize_first_block();

        let operator = 1;
        let staker = 2;
        let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");

        // Register provider, stake it, unstake a bit
        assert_register_provider(operator, &provider_id,200);
        assert_stake(staker, &provider_id, 100);
        assert_unstake(staker, &provider_id, 10);

        // Unregister provider and verify that stake & unstake no longer work
        assert_unregister(operator, &provider_id);

        assert_noop!(
            DapiStaking::stake(Origin::signed(staker), provider_id.clone(), 100),
            Error::<TestRuntime>::NotOperatedProvider
        );
        assert_noop!(
            DapiStaking::unstake(Origin::signed(staker), provider_id.clone(), 100),
            Error::<TestRuntime>::NotOperatedProvider
        );
    })
}

// #[test]
// fn withdraw_from_unregistered_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let dummy_operator = 2;
//         let staker_1 = 3;
//         let staker_2 = 4;
//         let staked_value_1 = 150;
//         let staked_value_2 = 330;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         let dummy_provider_id = MockSmartContract::Evm(H160::repeat_byte(0x05));
//
//         // Register both providers and stake them
//         assert_register_provider(operator, &provider_id,200);
//         assert_register_provider(dummy_operator, &dummy_provider_id);
//         assert_stake(staker_1, &provider_id, staked_value_1);
//         assert_stake(staker_2, &provider_id, staked_value_2);
//
//         // This provider will just exist so it helps us with testing ledger content
//         assert_stake(staker_1, &dummy_provider_id, staked_value_1);
//
//         // Advance eras. This will accumulate some rewards.
//         advance_to_era(5);
//
//         assert_unregister(operator, &provider_id);
//
//         // Claim all past rewards
//         for era in 1..DapiStaking::current_era() {
//             assert_claim_staker(staker_1, &provider_id);
//             assert_claim_staker(staker_2, &provider_id);
//             assert_claim_dapp(&provider_id, era);
//         }
//
//         // Unbond everything from the provider.
//         assert_withdraw_from_unregistered(staker_1, &provider_id);
//         assert_withdraw_from_unregistered(staker_2, &provider_id);
//
//         // No additional claim ops should be possible
//         assert_noop!(
//             DapiStaking::claim_staker(Origin::signed(staker_1), provider_id.clone()),
//             Error::<TestRuntime>::NotStakedContract
//         );
//         assert_noop!(
//             DapiStaking::claim_staker(Origin::signed(staker_2), provider_id.clone()),
//             Error::<TestRuntime>::NotStakedContract
//         );
//         assert_noop!(
//             DapiStaking::claim_dapp(
//                 Origin::signed(operator),
//                 provider_id.clone(),
//                 DapiStaking::current_era()
//             ),
//             Error::<TestRuntime>::NotOperatedProvider
//         );
//     })
// }
//
// #[test]
// fn withdraw_from_unregistered_when_provider_doesnt_exist() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_noop!(
//             DapiStaking::withdraw_from_unregistered(Origin::signed(1), provider_id),
//             Error::<TestRuntime>::NotOperatedProvider
//         );
//     })
// }
//
// #[test]
// fn withdraw_from_unregistered_when_provider_is_still_registered() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(operator, &provider_id,200);
//
//         assert_noop!(
//             DapiStaking::withdraw_from_unregistered(Origin::signed(1), provider_id),
//             Error::<TestRuntime>::NotUnregisteredContract
//         );
//     })
// }
//
// #[test]
// fn withdraw_from_unregistered_when_nothing_is_staked() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(operator, &provider_id,200);
//
//         let staker = 2;
//         let no_staker = 3;
//         assert_stake(staker, &provider_id, 100);
//
//         assert_unregister(operator, &provider_id);
//
//         // No staked amount so call should fail.
//         assert_noop!(
//             DapiStaking::withdraw_from_unregistered(Origin::signed(no_staker), provider_id),
//             Error::<TestRuntime>::NotStakedContract
//         );
//
//         // Call should fail if called twice since no staked funds remain.
//         assert_withdraw_from_unregistered(staker, &provider_id);
//         assert_noop!(
//             DapiStaking::withdraw_from_unregistered(Origin::signed(staker), provider_id),
//             Error::<TestRuntime>::NotStakedContract
//         );
//     })
// }
//
// #[test]
// fn withdraw_from_unregistered_when_unclaimed_rewards_remaing() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(operator, &provider_id,200);
//
//         let staker = 2;
//         assert_stake(staker, &provider_id, 100);
//
//         // Advance eras. This will accumulate some rewards.
//         advance_to_era(5);
//
//         assert_unregister(operator, &provider_id);
//
//         for _ in 1..DapiStaking::current_era() {
//             assert_noop!(
//                 DapiStaking::withdraw_from_unregistered(Origin::signed(staker), provider_id),
//                 Error::<TestRuntime>::UnclaimedRewardsRemaining
//             );
//             assert_claim_staker(staker, &provider_id);
//         }
//
//         // Withdraw should work after all rewards have been claimed
//         assert_withdraw_from_unregistered(staker, &provider_id);
//     })
// }
//
// #[test]
// fn stake_different_eras_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let staker_id = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(20, &provider_id);
//
//         // initially, storage values should be None
//         let current_era = DapiStaking::current_era();
//         assert!(DapiStaking::provider_stake_info(&provider_id, current_era).is_none());
//
//         assert_stake(staker_id, &provider_id, 100);
//
//         advance_to_era(current_era + 2);
//
//         // Stake and bond again on the same provider but using a different amount.
//         assert_stake(staker_id, &provider_id, 300);
//     })
// }
//
// #[test]
// fn stake_two_different_providers_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let staker_id = 1;
//         let first_provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         let second_provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000002");
//
//         // Insert providers under registered providers. Don't use the staker Id.
//         assert_register_provider(5, &first_provider_id);
//         assert_register_provider(6, &second_provider_id);
//
//         // Stake on both providers.
//         assert_stake(staker_id, &first_provider_id, 100);
//         assert_stake(staker_id, &second_provider_id, 300);
//     })
// }
//
// #[test]
// fn stake_two_stakers_one_provider_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let first_staker_id = 1;
//         let second_staker_id = 2;
//         let first_stake_value = 50;
//         let second_stake_value = 235;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         // Insert a provider under registered providers.
//         assert_register_provider(10, &provider_id);
//
//         // Both stakers stake on the same provider, expect a pass.
//         assert_stake(first_staker_id, &provider_id, first_stake_value);
//         assert_stake(second_staker_id, &provider_id, second_stake_value);
//     })
// }
//
// #[test]
// fn stake_different_value_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let staker_id = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         // Insert a provider under registered providers.
//         assert_register_provider(20, &provider_id);
//
//         // Bond&stake almost the entire available balance of the staker.
//         let staker_free_balance =
//             Balances::free_balance(&staker_id).saturating_sub(MINIMUM_REMAINING_AMOUNT);
//         assert_stake(staker_id, &provider_id, staker_free_balance - 1);
//
//         // Bond&stake again with less than existential deposit but this time expect a pass
//         // since we're only increasing the already staked amount.
//         assert_stake(staker_id, &provider_id, 1);
//
//         // Bond&stake more than what's available in funds. Verify that only what's available is bonded&staked.
//         let staker_id = 2;
//         let staker_free_balance = Balances::free_balance(&staker_id);
//         assert_stake(staker_id, &provider_id, staker_free_balance + 1);
//
//         // Verify the minimum transferable amount of stakers account
//         let transferable_balance =
//             Balances::free_balance(&staker_id) - Ledger::<TestRuntime>::get(staker_id).locked;
//         assert_eq!(MINIMUM_REMAINING_AMOUNT, transferable_balance);
//
//         // Bond&stake some amount, a bit less than free balance
//         let staker_id = 3;
//         let staker_free_balance =
//             Balances::free_balance(&staker_id).saturating_sub(MINIMUM_REMAINING_AMOUNT);
//         assert_stake(staker_id, &provider_id, staker_free_balance - 200);
//
//         // Try to bond&stake more than we have available (since we already locked most of the free balance).
//         assert_stake(staker_id, &provider_id, 500);
//     })
// }
//
// #[test]
// fn stake_on_unregistered_provider_fails() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let staker_id = 1;
//         let stake_value = 100;
//
//         // Check not registered provider. Expect an error.
//         let evm_provider = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_noop!(
//             DapiStaking::stake(Origin::signed(staker_id), evm_provider, stake_value),
//             Error::<TestRuntime>::NotOperatedProvider
//         );
//     })
// }
//
// #[test]
// fn stake_insufficient_value() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//         let staker_id = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         // Insert a provider under registered providers.
//         assert_register_provider(20, &provider_id);
//
//         // If user tries to make an initial bond&stake with less than minimum amount, raise an error.
//         assert_noop!(
//             DapiStaking::stake(
//                 Origin::signed(staker_id),
//                 provider_id.clone(),
//                 MINIMUM_STAKING_AMOUNT - 1
//             ),
//             Error::<TestRuntime>::InsufficientValue
//         );
//
//         // Now bond&stake the entire stash so we lock all the available funds.
//         let staker_free_balance = Balances::free_balance(&staker_id);
//         assert_stake(staker_id, &provider_id, staker_free_balance);
//
//         // Now try to bond&stake some additional funds and expect an error since we cannot bond&stake 0.
//         assert_noop!(
//             DapiStaking::stake(Origin::signed(staker_id), provider_id.clone(), 1),
//             Error::<TestRuntime>::StakingWithNoValue
//         );
//     })
// }
//
// #[test]
// fn stake_too_many_stakers_per_provider() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         // Insert a provider under registered providers.
//         assert_register_provider(10, &provider_id);
//
//         // Stake with MAX_NUMBER_OF_STAKERS on the same provider. It must work.
//         for staker_id in 1..=MAX_NUMBER_OF_STAKERS {
//             assert_stake(staker_id.into(), &provider_id, 100);
//         }
//
//         // Now try to stake with an additional staker and expect an error.
//         assert_noop!(
//             DapiStaking::stake(
//                 Origin::signed((1 + MAX_NUMBER_OF_STAKERS).into()),
//                 provider_id.clone(),
//                 100
//             ),
//             Error::<TestRuntime>::MaxNumberOfStakersExceeded
//         );
//     })
// }
//
// #[test]
// fn stake_too_many_era_stakes() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let staker_id = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         // Insert a provider under registered providers.
//         assert_register_provider(10, &provider_id);
//
//         // Stake with MAX_NUMBER_OF_STAKERS - 1 on the same provider. It must work.
//         let start_era = DapiStaking::current_era();
//         for offset in 1..MAX_ERA_STAKE_VALUES {
//             assert_stake(staker_id, &provider_id, 100);
//             advance_to_era(start_era + offset);
//         }
//
//         // Now try to stake with an additional staker and expect an error.
//         assert_noop!(
//             DapiStaking::stake(Origin::signed(staker_id), provider_id, 100),
//             Error::<TestRuntime>::TooManyEraStakeValues
//         );
//     })
// }
//
// #[test]
// fn unstake_multiple_time_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let staker_id = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         let original_staked_value = 300 + MINIMUM_STAKING_AMOUNT;
//         let old_era = DapiStaking::current_era();
//
//         // Insert a provider under registered providers, bond&stake it.
//         assert_register_provider(10, &provider_id);
//         assert_stake(staker_id, &provider_id, original_staked_value);
//         advance_to_era(old_era + 1);
//
//         // Unstake such an amount so there will remain staked funds on the provider
//         let unstaked_value = 100;
//         assert_unstake(staker_id, &provider_id, unstaked_value);
//
//         // Unbond yet again, but don't advance era
//         // Unstake such an amount so there will remain staked funds on the provider
//         let unstaked_value = 50;
//         assert_unstake(staker_id, &provider_id, unstaked_value);
//     })
// }
//
// #[test]
// fn unstake_value_below_staking_threshold() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let staker_id = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         let first_value_to_unstake = 300;
//         let staked_value = first_value_to_unstake + MINIMUM_STAKING_AMOUNT;
//
//         // Insert a provider under registered providers, bond&stake it.
//         assert_register_provider(10, &provider_id);
//         assert_stake(staker_id, &provider_id, staked_value);
//
//         // Unstake such an amount that exactly minimum staking amount will remain staked.
//         assert_unstake(staker_id, &provider_id, first_value_to_unstake);
//
//         // Unstake 1 token and expect that the entire staked amount will be unstaked.
//         assert_unstake(staker_id, &provider_id, 1);
//     })
// }
//
// #[test]
// fn unstake_in_different_eras() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let (first_staker_id, second_staker_id) = (1, 2);
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         let staked_value = 500;
//
//         // Insert a provider under registered providers, bond&stake it with two different stakers.
//         assert_register_provider(10, &provider_id);
//         assert_stake(first_staker_id, &provider_id, staked_value);
//         assert_stake(second_staker_id, &provider_id, staked_value);
//
//         // Advance era, unbond&withdraw with first staker, verify that it was successful
//         advance_to_era(DapiStaking::current_era() + 10);
//         let current_era = DapiStaking::current_era();
//         assert_unstake(first_staker_id, &provider_id, 100);
//
//         // Advance era, unbond with second staker and verify storage values are as expected
//         advance_to_era(current_era + 10);
//         assert_unstake(second_staker_id, &provider_id, 333);
//     })
// }
//
// #[test]
// fn unstake_calls_in_same_era_can_exceed_max_chunks() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         let staker = 1;
//         assert_stake(staker, &provider_id, 200 * MAX_UNLOCKING_CHUNKS as Balance);
//
//         // Ensure that we can unbond up to a limited amount of time.
//         for _ in 0..MAX_UNLOCKING_CHUNKS * 2 {
//             assert_unstake(1, &provider_id, 10);
//             assert_eq!(1, Ledger::<TestRuntime>::get(&staker).unbonding_info.len());
//         }
//     })
// }
//
// #[test]
// fn unstake_with_zero_value_is_not_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         assert_noop!(
//             DapiStaking::unstake(Origin::signed(1), provider_id, 0),
//             Error::<TestRuntime>::UnstakingWithNoValue
//         );
//     })
// }
//
// #[test]
// fn unstake_on_not_operated_provider_is_not_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_noop!(
//             DapiStaking::unstake(Origin::signed(1), provider_id, 100),
//             Error::<TestRuntime>::NotOperatedProvider
//         );
//     })
// }
//
// #[test]
// fn unstake_too_many_unlocking_chunks_is_not_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         let staker = 1;
//         let unstake_amount = 10;
//         let stake_amount =
//             MINIMUM_STAKING_AMOUNT * 10 + unstake_amount * MAX_UNLOCKING_CHUNKS as Balance;
//
//         assert_stake(staker, &provider_id, stake_amount);
//
//         // Ensure that we can unbond up to a limited amount of time.
//         for _ in 0..MAX_UNLOCKING_CHUNKS {
//             advance_to_era(DapiStaking::current_era() + 1);
//             assert_unstake(staker, &provider_id, unstake_amount);
//         }
//
//         // Ensure that we're at the max but can still add new chunks since it should be merged with the existing one
//         assert_eq!(
//             MAX_UNLOCKING_CHUNKS,
//             DapiStaking::ledger(&staker).unbonding_info.len()
//         );
//         assert_unstake(staker, &provider_id, unstake_amount);
//
//         // Ensure that further unbonding attempts result in an error.
//         advance_to_era(DapiStaking::current_era() + 1);
//         assert_noop!(
//             DapiStaking::unstake(
//                 Origin::signed(staker),
//                 provider_id.clone(),
//                 unstake_amount
//             ),
//             Error::<TestRuntime>::TooManyUnlockingChunks,
//         );
//     })
// }
//
// #[test]
// fn unstake_on_not_staked_provider_is_not_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         assert_noop!(
//             DapiStaking::unstake(Origin::signed(1), provider_id, 10),
//             Error::<TestRuntime>::NotStakedContract,
//         );
//     })
// }
//
// #[test]
// fn unstake_too_many_era_stakes() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let staker_id = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         // Fill up the `EraStakes` vec
//         let start_era = DapiStaking::current_era();
//         for offset in 1..MAX_ERA_STAKE_VALUES {
//             assert_stake(staker_id, &provider_id, 100);
//             advance_to_era(start_era + offset);
//         }
//
//         // At this point, we have max allowed amount of `EraStake` values so we cannot create
//         // an additional one.
//         assert_noop!(
//             DapiStaking::unstake(Origin::signed(staker_id), provider_id, 10),
//             Error::<TestRuntime>::TooManyEraStakeValues
//         );
//     })
// }
//
// #[ignore]
// #[test]
// fn unstake_with_no_chunks_allowed() {
//     // UT can be used to verify situation when MaxUnlockingChunks = 0. Requires mock modification.
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         // Sanity check
//         assert_eq!(<TestRuntime as Config>::MaxUnlockingChunks::get(), 0);
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         let staker_id = 1;
//         assert_stake(staker_id, &provider_id, 100);
//
//         assert_noop!(
//             DapiStaking::unstake(Origin::signed(staker_id), provider_id.clone(), 20),
//             Error::<TestRuntime>::TooManyUnlockingChunks,
//         );
//     })
// }
//
// #[test]
// fn withdraw_unbonded_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         let staker_id = 1;
//         assert_stake(staker_id, &provider_id, 1000);
//
//         let first_unbond_value = 75;
//         let second_unbond_value = 39;
//         let initial_era = DapiStaking::current_era();
//
//         // Unbond some amount in the initial era
//         assert_unstake(staker_id, &provider_id, first_unbond_value);
//
//         // Advance one era and then unbond some more
//         advance_to_era(initial_era + 1);
//         assert_unstake(staker_id, &provider_id, second_unbond_value);
//
//         // Now advance one era before first chunks finishes the unbonding process
//         advance_to_era(initial_era + UNBONDING_PERIOD - 1);
//         assert_noop!(
//             DapiStaking::withdraw_unbonded(Origin::signed(staker_id)),
//             Error::<TestRuntime>::NothingToWithdraw
//         );
//
//         // Advance one additional era and expect that the first chunk can be withdrawn
//         advance_to_era(DapiStaking::current_era() + 1);
//         assert_ok!(DapiStaking::withdraw_unbonded(Origin::signed(staker_id),));
//         System::assert_last_event(mock::Event::DapiStaking(Event::Withdrawn(
//             staker_id,
//             first_unbond_value,
//         )));
//
//         // Advance one additional era and expect that the first chunk can be withdrawn
//         advance_to_era(DapiStaking::current_era() + 1);
//         assert_ok!(DapiStaking::withdraw_unbonded(Origin::signed(staker_id),));
//         System::assert_last_event(mock::Event::DapiStaking(Event::Withdrawn(
//             staker_id,
//             second_unbond_value,
//         )));
//
//         // Advance one additional era but since we have nothing else to withdraw, expect an error
//         advance_to_era(initial_era + UNBONDING_PERIOD - 1);
//         assert_noop!(
//             DapiStaking::withdraw_unbonded(Origin::signed(staker_id)),
//             Error::<TestRuntime>::NothingToWithdraw
//         );
//     })
// }
//
// #[test]
// fn withdraw_unbonded_full_vector_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         let staker_id = 1;
//         assert_stake(staker_id, &provider_id, 1000);
//
//         // Repeatedly start unbonding and advance era to create unlocking chunks
//         let init_unbonding_amount = 15;
//         for x in 1..=MAX_UNLOCKING_CHUNKS {
//             assert_unstake(staker_id, &provider_id, init_unbonding_amount * x as u128);
//             advance_to_era(DapiStaking::current_era() + 1);
//         }
//
//         // Now clean up all that are eligible for cleanu-up
//         assert_withdraw_unbonded(staker_id);
//
//         // This is a sanity check for the test. Some chunks should remain, otherwise test isn't testing realistic unbonding period.
//         assert!(!Ledger::<TestRuntime>::get(&staker_id)
//             .unbonding_info
//             .is_empty());
//
//         while !Ledger::<TestRuntime>::get(&staker_id)
//             .unbonding_info
//             .is_empty()
//         {
//             advance_to_era(DapiStaking::current_era() + 1);
//             assert_withdraw_unbonded(staker_id);
//         }
//     })
// }
//
// #[test]
// fn withdraw_unbonded_no_value_is_not_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         assert_noop!(
//             DapiStaking::withdraw_unbonded(Origin::signed(1)),
//             Error::<TestRuntime>::NothingToWithdraw,
//         );
//     })
// }
//
// #[ignore]
// #[test]
// fn withdraw_unbonded_no_unbonding_period() {
//     // UT can be used to verify situation when UnbondingPeriod = 0. Requires mock modification.
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         // Sanity check
//         assert_eq!(<TestRuntime as Config>::UnbondingPeriod::get(), 0);
//
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         assert_register_provider(10, &provider_id);
//
//         let staker_id = 1;
//         assert_stake(staker_id, &provider_id, 100);
//         assert_unstake(staker_id, &provider_id, 20);
//
//         // Try to withdraw but expect an error since current era hasn't passed yet
//         assert_noop!(
//             DapiStaking::withdraw_unbonded(Origin::signed(staker_id)),
//             Error::<TestRuntime>::NothingToWithdraw,
//         );
//
//         // Advance an era and expect successful withdrawal
//         advance_to_era(DapiStaking::current_era() + 1);
//         assert_withdraw_unbonded(staker_id);
//     })
// }
//
// #[test]
// fn claim_not_staked_provider() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let staker = 2;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         assert_register_provider(operator, &provider_id,200);
//
//         assert_noop!(
//             DapiStaking::claim_staker(Origin::signed(staker), provider_id),
//             Error::<TestRuntime>::NotStakedContract
//         );
//
//         advance_to_era(DapiStaking::current_era() + 1);
//         assert_noop!(
//             DapiStaking::claim_dapp(Origin::signed(operator), provider_id, 1),
//             Error::<TestRuntime>::NotStakedContract
//         );
//     })
// }
//
// #[test]
// fn claim_not_operated_provider() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let staker = 2;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         assert_register_provider(operator, &provider_id,200);
//         assert_stake(staker, &provider_id, 100);
//
//         // Advance one era and unregister the provider
//         advance_to_era(DapiStaking::current_era() + 1);
//         assert_unregister(operator, &provider_id);
//
//         // First claim should pass but second should fail because provider was unregistered
//         assert_claim_staker(staker, &provider_id);
//         assert_noop!(
//             DapiStaking::claim_staker(Origin::signed(staker), provider_id),
//             Error::<TestRuntime>::NotOperatedProvider
//         );
//
//         assert_claim_dapp(&provider_id, 1);
//         assert_noop!(
//             DapiStaking::claim_dapp(Origin::signed(operator), provider_id, 2),
//             Error::<TestRuntime>::NotOperatedProvider
//         );
//     })
// }
//
// #[test]
// fn claim_invalid_era() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let staker = 2;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         let start_era = DapiStaking::current_era();
//         assert_register_provider(operator, &provider_id,200);
//         assert_stake(staker, &provider_id, 100);
//         advance_to_era(start_era + 5);
//
//         for era in start_era..DapiStaking::current_era() {
//             assert_claim_staker(staker, &provider_id);
//             assert_claim_dapp(&provider_id, era);
//         }
//
//         assert_noop!(
//             DapiStaking::claim_staker(Origin::signed(staker), provider_id),
//             Error::<TestRuntime>::EraOutOfBounds
//         );
//         assert_noop!(
//             DapiStaking::claim_dapp(
//                 Origin::signed(operator),
//                 provider_id,
//                 DapiStaking::current_era()
//             ),
//             Error::<TestRuntime>::EraOutOfBounds
//         );
//     })
// }
//
// #[test]
// fn claim_dapp_same_era_twice() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let staker = 2;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         let start_era = DapiStaking::current_era();
//         assert_register_provider(operator, &provider_id,200);
//         assert_stake(staker, &provider_id, 100);
//         advance_to_era(start_era + 1);
//
//         assert_claim_dapp(&provider_id, start_era);
//         assert_noop!(
//             DapiStaking::claim_dapp(Origin::signed(operator), provider_id, start_era),
//             Error::<TestRuntime>::AlreadyClaimedInThisEra
//         );
//     })
// }
//
// #[test]
// fn claim_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let first_operator = 1;
//         let second_operator = 2;
//         let first_staker = 3;
//         let second_staker = 4;
//         let first_provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//         let second_provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000002");
//
//         let start_era = DapiStaking::current_era();
//
//         // Prepare a scenario with different stakes
//
//         assert_register_provider(first_operator, &first_provider_id);
//         assert_register_provider(second_operator, &second_provider_id);
//         assert_stake(first_staker, &first_provider_id, 100);
//         assert_stake(second_staker, &first_provider_id, 45);
//
//         // Just so ratio isn't 100% in favor of the first provider
//         assert_stake(first_staker, &second_provider_id, 33);
//         assert_stake(second_staker, &second_provider_id, 22);
//
//         let eras_advanced = 3;
//         advance_to_era(start_era + eras_advanced);
//
//         for x in 0..eras_advanced.into() {
//             assert_stake(first_staker, &first_provider_id, 20 + x * 3);
//             assert_stake(second_staker, &first_provider_id, 5 + x * 5);
//             advance_to_era(DapiStaking::current_era() + 1);
//         }
//
//         // Ensure that all past eras can be claimed
//         let current_era = DapiStaking::current_era();
//         for era in start_era..current_era {
//             assert_claim_staker(first_staker, &first_provider_id);
//             assert_claim_dapp(&first_provider_id, era);
//             assert_claim_staker(second_staker, &first_provider_id);
//         }
//
//         // Shouldn't be possible to claim current era.
//         // Also, previous claim calls should have claimed everything prior to current era.
//         assert_noop!(
//             DapiStaking::claim_staker(Origin::signed(first_staker), first_provider_id.clone()),
//             Error::<TestRuntime>::EraOutOfBounds
//         );
//         assert_noop!(
//             DapiStaking::claim_dapp(
//                 Origin::signed(first_operator),
//                 first_provider_id,
//                 current_era
//             ),
//             Error::<TestRuntime>::EraOutOfBounds
//         );
//     })
// }
//
// #[test]
// fn claim_after_unregister_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let staker = 2;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         let start_era = DapiStaking::current_era();
//         assert_register_provider(operator, &provider_id,200);
//         let stake_value = 100;
//         assert_stake(staker, &provider_id, stake_value);
//
//         // Advance few eras, then unstake everything
//         advance_to_era(start_era + 5);
//         assert_unstake(staker, &provider_id, stake_value);
//         let full_unstake_era = DapiStaking::current_era();
//         let number_of_staking_eras = full_unstake_era - start_era;
//
//         // Few eras pass, then staker stakes again
//         advance_to_era(DapiStaking::current_era() + 3);
//         let stake_value = 75;
//         let restake_era = DapiStaking::current_era();
//         assert_stake(staker, &provider_id, stake_value);
//
//         // Again, few eras pass then provider is unregistered
//         advance_to_era(DapiStaking::current_era() + 3);
//         assert_unregister(operator, &provider_id);
//         let unregister_era = DapiStaking::current_era();
//         let number_of_staking_eras = number_of_staking_eras + unregister_era - restake_era;
//         advance_to_era(DapiStaking::current_era() + 2);
//
//         // Ensure that staker can claim all the eras that he had an active stake
//         for _ in 0..number_of_staking_eras {
//             assert_claim_staker(staker, &provider_id);
//         }
//         assert_noop!(
//             DapiStaking::claim_staker(Origin::signed(staker), provider_id.clone()),
//             Error::<TestRuntime>::NotOperatedProvider
//         );
//
//         // Ensure the same for dapp reward
//         for era in start_era..unregister_era {
//             if era >= full_unstake_era && era < restake_era {
//                 assert_noop!(
//                     DapiStaking::claim_dapp(Origin::signed(operator), provider_id.clone(), era),
//                     Error::<TestRuntime>::NotStakedContract
//                 );
//             } else {
//                 assert_claim_dapp(&provider_id, era);
//             }
//         }
//     })
// }
//
// #[test]
// fn claim_dapp_with_zero_stake_periods_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         let operator = 1;
//         let staker = 2;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         // Prepare scenario: <staked eras><not staked eras><staked eras><not staked eras>
//
//         let start_era = DapiStaking::current_era();
//         assert_register_provider(operator, &provider_id);
//         let stake_value = 100;
//         assert_stake(staker, &provider_id, stake_value);
//
//         advance_to_era(start_era + 5);
//         let first_full_unstake_era = DapiStaking::current_era();
//         assert_unstake(staker, &provider_id, stake_value);
//
//         advance_to_era(DapiStaking::current_era() + 7);
//         let restake_era = DapiStaking::current_era();
//         assert_stake(staker, &provider_id, stake_value);
//
//         advance_to_era(DapiStaking::current_era() + 4);
//         let second_full_unstake_era = DapiStaking::current_era();
//         assert_unstake(staker, &provider_id, stake_value);
//         advance_to_era(DapiStaking::current_era() + 10);
//
//         // Ensure that first interval can be claimed
//         for era in start_era..first_full_unstake_era {
//             assert_claim_dapp(&provider_id, era);
//         }
//
//         // Ensure that the empty interval cannot be claimed
//         for era in first_full_unstake_era..restake_era {
//             assert_noop!(
//                 DapiStaking::claim_dapp(Origin::signed(operator), provider_id.clone(), era),
//                 Error::<TestRuntime>::NotStakedContract
//             );
//         }
//
//         // Ensure that second interval can be claimed
//         for era in restake_era..second_full_unstake_era {
//             assert_claim_dapp(&provider_id, era);
//         }
//
//         // Ensure no more claims are possible since provider was fully unstaked
//         assert_noop!(
//             DapiStaking::claim_dapp(
//                 Origin::signed(operator),
//                 provider_id.clone(),
//                 second_full_unstake_era
//             ),
//             Error::<TestRuntime>::NotStakedContract
//         );
//
//         // Now stake again and ensure provider can once again be claimed
//         let last_claim_era = DapiStaking::current_era();
//         assert_stake(staker, &provider_id, stake_value);
//         advance_to_era(last_claim_era + 1);
//         assert_claim_dapp(&provider_id, last_claim_era);
//     })
// }
//
// #[test]
// fn maintenance_mode_is_ok() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         assert_ok!(DapiStaking::ensure_pallet_enabled());
//         assert!(!PalletDisabled::<TestRuntime>::exists());
//
//         assert_ok!(DapiStaking::maintenance_mode(Origin::root(), true));
//         assert!(PalletDisabled::<TestRuntime>::exists());
//         System::assert_last_event(mock::Event::DapiStaking(Event::MaintenanceMode(true)));
//
//         let account = 1;
//         let provider_id = MockProvider(*b"00000000-0000-0000-0000-000000000001");
//
//         //
//         // 1
//         assert_noop!(
//             DapiStaking::register(Origin::signed(account), provider_id),
//             Error::<TestRuntime>::Disabled
//         );
//         assert_noop!(
//             DapiStaking::unregister(Origin::signed(account), provider_id),
//             Error::<TestRuntime>::Disabled
//         );
//         assert_noop!(
//             DapiStaking::withdraw_from_unregistered(Origin::signed(account), provider_id),
//             Error::<TestRuntime>::Disabled
//         );
//
//         //
//         // 2
//         assert_noop!(
//             DapiStaking::stake(Origin::signed(account), provider_id, 100),
//             Error::<TestRuntime>::Disabled
//         );
//         assert_noop!(
//             DapiStaking::unstake(Origin::signed(account), provider_id, 100),
//             Error::<TestRuntime>::Disabled
//         );
//         assert_noop!(
//             DapiStaking::claim_dapp(Origin::signed(account), provider_id, 5),
//             Error::<TestRuntime>::Disabled
//         );
//         assert_noop!(
//             DapiStaking::claim_staker(Origin::signed(account), provider_id),
//             Error::<TestRuntime>::Disabled
//         );
//         assert_noop!(
//             DapiStaking::withdraw_unbonded(Origin::signed(account)),
//             Error::<TestRuntime>::Disabled
//         );
//
//         //
//         // 3
//         assert_noop!(
//             DapiStaking::force_new_era(Origin::root()),
//             Error::<TestRuntime>::Disabled
//         );
//         assert_noop!(
//             DapiStaking::operator_pre_approval(Origin::root(), account),
//             Error::<TestRuntime>::Disabled
//         );
//         assert_noop!(
//             DapiStaking::enable_operator_pre_approval(Origin::root(), true),
//             Error::<TestRuntime>::Disabled
//         );
//         // shouldn't do anything since we're in maintenance mode
//         assert_eq!(DapiStaking::on_initialize(3), 0);
//
//         //
//         // 4
//         assert_ok!(DapiStaking::maintenance_mode(Origin::root(), false));
//         System::assert_last_event(mock::Event::DapiStaking(Event::MaintenanceMode(false)));
//         assert_register_provider(account, &provider_id);
//     })
// }
//
// #[test]
// fn maintenance_mode_no_change() {
//     ExternalityBuilder::build().execute_with(|| {
//         initialize_first_block();
//
//         // Expect an error since maintenance mode is already disabled
//         assert_ok!(DapiStaking::ensure_pallet_enabled());
//         assert_noop!(
//             DapiStaking::maintenance_mode(Origin::root(), false),
//             Error::<TestRuntime>::NoMaintenanceModeChange
//         );
//
//         // Same for the case when maintenance mode is already enabled
//         assert_ok!(DapiStaking::maintenance_mode(Origin::root(), true));
//         assert_noop!(
//             DapiStaking::maintenance_mode(Origin::root(), true),
//             Error::<TestRuntime>::NoMaintenanceModeChange
//         );
//     })
// }
//
// #[test]
// fn dev_stakers_split_util() {
//     let base_stakers_reward = 7 * 11 * 13 * 17;
//     let base_dapps_reward = 19 * 23 * 31;
//     let staked_on_provider = 123456;
//     let total_staked = staked_on_provider * 3;
//
//     // Prepare structs
//     let staking_points = ContractStakeInfo::<Balance> {
//         total: staked_on_provider,
//         number_of_stakers: 10,
//         provider_reward_claimed: false,
//     };
//     let era_info = EraInfo::<Balance> {
//         rewards: RewardInfo {
//             dapi: base_dapps_reward,
//             stakers: base_stakers_reward,
//         },
//         staked: total_staked,
//         locked: total_staked,
//     };
//
//     let (dev_reward, stakers_reward) = DapiStaking::dev_stakers_split(&staking_points, &era_info);
//
//     let provider_stake_ratio = Perbill::from_rational(staked_on_provider, total_staked);
//     let calculated_stakers_reward = provider_stake_ratio * base_stakers_reward;
//     let calculated_dev_reward = provider_stake_ratio * base_dapps_reward;
//     assert_eq!(calculated_dev_reward, dev_reward);
//     assert_eq!(calculated_stakers_reward, stakers_reward);
//
//     assert_eq!(
//         calculated_stakers_reward + calculated_dev_reward,
//         dev_reward + stakers_reward
//     );
// }
