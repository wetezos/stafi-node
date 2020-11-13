// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::prelude::*;
use codec::{Codec, Encode, Decode};
use frame_support::{
    Parameter, decl_error, decl_event, decl_module, decl_storage,
    dispatch::DispatchResult,
    ensure,
    traits::{Currency, Get, ExistenceRequirement::KeepAlive},
};
use frame_system::{self as system, ensure_signed, ensure_root};
use sp_runtime::traits::{Zero, AccountIdConversion, CheckedSub, CheckedAdd, SaturatedConversion, Saturating};
use sp_runtime::{ModuleId};
use pallet_staking::{self as staking, RewardDestination, StakingLedger, EraIndex};
use sp_arithmetic::{helpers_128bit::multiply_by_rational};
use rtoken_balances::{RTokenIdentifier, traits::{Currency as RCurrency}};

const POOL_ID_1: ModuleId = ModuleId(*b"rFISpot1");

type BalanceOf<T> = staking::BalanceOf<T>;
// type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;
type RBalanceOf<T> = <<T as Trait>::RCurrency as RCurrency<<T as frame_system::Trait>::AccountId>>::Balance;

// #[derive(Encode, Decode, Clone, PartialEq, Default)]
// pub struct LiquidityStakeData<AccountId, Balance: HasCompact, EraIndex> {
// 	// pool
// 	pub pool: AccountId,
// 	// Token data of stake
// 	pub stake_amount: Balance,
// 	// issue block
// 	pub _era: BlockNumber
// }

pub trait Trait: system::Trait + staking::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    // type Currency: Currency<Self::AccountId>;

    type RCurrency: RCurrency<Self::AccountId>;

    type Symbol: Get<RTokenIdentifier>;
}

decl_event! {
    pub enum Event<T> where
        Balance = BalanceOf<T>,
        <T as frame_system::Trait>::AccountId
    {
        /// liquidity stake record
        LiquidityStake(AccountId, Balance),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Got an overflow after adding
        Overflow,
        /// StakeZero
        StakeZero,
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as FisStaking {
        pub TotalStaked get(fn total_staked): BalanceOf<T>;

        pub LiquidityStaked get(fn liquidity_staked): map hasher(blake2_128_concat) T::AccountId => Option<BalanceOf<T>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        const Symbol: RTokenIdentifier = T::Symbol::get();

        fn deposit_event() = default;

        #[weight = 195_000_000]
        pub fn liquidity_stake(origin, value: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(value > Zero::zero(), Error::<T>::StakeZero);
            let total_staked = <TotalStaked<T>>::get();
            let new_total_staked = total_staked.checked_add(&value).ok_or(Error::<T>::Overflow)?;

            let total_issuance = T::RCurrency::total_issuance();
            let mut rvalue: RBalanceOf<T> = Zero::zero();
            if total_staked == Zero::zero() || total_issuance == Zero::zero() {
                rvalue = value.saturated_into::<u128>().saturated_into::<RBalanceOf<T>>();
            } else {
                let a = value.saturated_into::<u128>();
                let b = total_issuance.saturated_into::<u128>();
                let c = total_staked.saturated_into::<u128>();
                rvalue = multiply_by_rational(a, b, c).ok().unwrap().saturated_into::<RBalanceOf<T>>();
            }
            let new_total_issuance = total_issuance.checked_add(&rvalue).ok_or(Error::<T>::Overflow)?;
            let mut new_staked: BalanceOf<T> = value;
            if let Some(staked) = <LiquidityStaked<T>>::get(&who) {
                new_staked = staked.checked_add(&value).ok_or(Error::<T>::Overflow)?;
            }

            let pot = Self::account_id_1();
            T::Currency::transfer(&who, &pot, value.into(), KeepAlive)?;
            T::RCurrency::deposit_into(&who, rvalue);
            T::RCurrency::issue(rvalue);
            <TotalStaked<T>>::put(new_total_staked);
            <LiquidityStaked<T>>::insert(&who, new_staked);
            
            // Self::bond_extra(&pot, value.into());

            Self::deposit_event(RawEvent::LiquidityStake(who, new_staked));

            Ok(())
        }

        #[weight = 100_000_000]
        pub fn bond(origin) -> DispatchResult {
			ensure_root(origin)?;

            let stash: T::AccountId = Self::account_id_1();

            if staking::Bonded::<T>::contains_key(&stash) {
                Err(staking::Error::<T>::AlreadyBonded)?
            }

            if staking::Ledger::<T>::contains_key(&stash) {
                Err(staking::Error::<T>::AlreadyPaired)?
            }

            staking::Bonded::<T>::insert(&stash, &stash);
            staking::Payee::<T>::insert(&stash, RewardDestination::Staked);

            system::Module::<T>::inc_ref(&stash);

            let value = Zero::zero();
			// Self::deposit_event(RawEvent::Bonded(stash.clone(), value));
			let item = StakingLedger {
				stash,
				total: value,
				active: value,
				unlocking: vec![],
				claimed_rewards: vec![],
            };

            let controller: T::AccountId = Self::account_id_1();
            staking::Module::<T>::update_ledger(&controller, &item);
            Ok(())
		}
    }
}

impl<T: Trait> Module<T> {
    /// Provides an AccountId for the pallet.
    /// This is used both as an origin check and deposit/withdrawal account.
    pub fn account_id_1() -> T::AccountId {
        POOL_ID_1.into_account()
    }

    fn bond_extra(stash: &T::AccountId, max_additional: BalanceOf<T>) {
		let controller = <staking::Module<T>>::bonded(&stash).unwrap();
		let mut ledger = <staking::Module<T>>::ledger(&controller).unwrap();

        let stash_balance = T::Currency::free_balance(&stash);
        // let stash_balance = <T as staking::Trait>::Currency::free_balance(&stash);

		if let Some(extra) = stash_balance.checked_sub(&ledger.total) {
			let extra = extra.min(max_additional);
			ledger.total += extra;
			ledger.active += extra;
			staking::Module::<T>::update_ledger(&controller, &ledger);
		}
    }
}

