// Copyright 2019-2020 Stafi Protocol.
// This file is part of Stafi.

// Stafi is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Stafi.  If not, see <http://www.gnu.org/licenses/>.

use codec::{Encode, Joiner};
use frame_support::{
	StorageValue, StorageMap,
	traits::Currency,
	weights::{GetDispatchInfo, constants::ExtrinsicBaseWeight, IdentityFee, WeightToFeePolynomial},
};
use sp_core::NeverNativeValue;
use sp_runtime::{Perbill, FixedPointNumber};
use node_runtime::{
	CheckedExtrinsic, Call, Runtime, Balances, TransactionPayment, Multiplier,
	TransactionByteFee,
	constants::currency::*,
};
use node_primitives::Balance;
use node_testing::keyring::*;

pub mod common;
use self::common::{*, sign};

#[test]
fn fee_multiplier_increases_and_decreases_on_big_weight() {
	let mut t = new_test_ext(compact_code_unwrap(), false);

	// initial fee multiplier must be one.
	let mut prev_multiplier = Multiplier::one();

	t.execute_with(|| {
		assert_eq!(TransactionPayment::next_fee_multiplier(), prev_multiplier);
	});

	let mut tt = new_test_ext(compact_code_unwrap(), false);

	// big one in terms of weight.
	let block1 = construct_block(
		&mut tt,
		1,
		GENESIS_HASH.into(),
		vec![
			CheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(pallet_timestamp::Call::set(42 * 1000)),
			},
			CheckedExtrinsic {
				signed: Some((charlie(), signed_extra(0, 0))),
				function: Call::System(frame_system::Call::fill_block(Perbill::from_percent(60))),
			}
		]
	);

	// small one in terms of weight.
	let block2 = construct_block(
		&mut tt,
		2,
		block1.1.clone(),
		vec![
			CheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(pallet_timestamp::Call::set(52 * 1000)),
			},
			CheckedExtrinsic {
				signed: Some((charlie(), signed_extra(1, 0))),
				function: Call::System(frame_system::Call::remark(vec![0; 1])),
			}
		]
	);

	println!(
		"++ Block 1 size: {} / Block 2 size {}",
		block1.0.encode().len(),
		block2.0.encode().len(),
	);

	// execute a big block.
	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block1.0,
		true,
		None,
	).0.unwrap();

	// weight multiplier is increased for next block.
	t.execute_with(|| {
		let fm = TransactionPayment::next_fee_multiplier();
		println!("After a big block: {:?} -> {:?}", prev_multiplier, fm);
		assert!(fm > prev_multiplier);
		prev_multiplier = fm;
	});

	// execute a big block.
	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block2.0,
		true,
		None,
	).0.unwrap();

	// weight multiplier is increased for next block.
	t.execute_with(|| {
		let fm = TransactionPayment::next_fee_multiplier();
		println!("After a small block: {:?} -> {:?}", prev_multiplier, fm);
		assert!(fm < prev_multiplier);
	});
}

#[test]
fn transaction_fee_is_correct() {
	// This uses the exact values of substrate-node.
	//
	// weight of transfer call as of now: 1_000_000
	// if weight of the cheapest weight would be 10^7, this would be 10^9, which is:
	//   - 1 MILLICENTS in substrate node.
	//   - 1 milli-dot based on current polkadot runtime.
	// (this baed on assigning 0.1 CENT to the cheapest tx with `weight = 100`)
	let mut t = new_test_ext(compact_code_unwrap(), false);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(alice()),
		(0u32, 0u8, 100 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS).encode()
	);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(bob()),
		(0u32, 0u8, 10 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS, 0 * DOLLARS).encode()
	);
	t.insert(
		<pallet_balances::TotalIssuance<Runtime>>::hashed_key().to_vec(),
		(110 * DOLLARS).encode()
	);
	t.insert(<frame_system::BlockHash<Runtime>>::hashed_key_for(0), vec![0u8; 32]);

	let tip = 1_000_000;
	let xt = sign(CheckedExtrinsic {
		signed: Some((alice(), signed_extra(0, tip))),
		function: Call::Balances(default_transfer_call()),
	});

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_initialize_block",
		&vec![].and(&from_block_number(1u32)),
		true,
		None,
	).0;

	assert!(r.is_ok());
	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"BlockBuilder_apply_extrinsic",
		&vec![].and(&xt.clone()),
		true,
		None,
	).0;
	assert!(r.is_ok());

	t.execute_with(|| {
		assert_eq!(Balances::total_balance(&bob()), (10 + 69) * DOLLARS);
		// Components deducted from alice's balances:
		// - Base fee
		// - Weight fee
		// - Length fee
		// - Tip
		// - Creation-fee of bob's account.
		let mut balance_alice = (100 - 69) * DOLLARS;

		let base_weight = ExtrinsicBaseWeight::get();
		let base_fee = IdentityFee::<Balance>::calc(&base_weight);

		let length_fee = TransactionByteFee::get() * (xt.clone().encode().len() as Balance);
		balance_alice -= length_fee;

		let weight = default_transfer_call().get_dispatch_info().weight;
		let weight_fee = IdentityFee::<Balance>::calc(&weight);

		// we know that weight to fee multiplier is effect-less in block 1.
		// current weight of transfer = 200_000_000
		// Linear weight to fee is 1:1 right now (1 weight = 1 unit of balance)
		assert_eq!(weight_fee, weight as Balance);
		balance_alice -= base_fee;
		balance_alice -= weight_fee;
		balance_alice -= tip;

		assert_eq!(Balances::total_balance(&alice()), balance_alice);
	});
}

#[test]
#[should_panic]
#[cfg(feature = "stress-test")]
fn block_weight_capacity_report() {
	// Just report how many transfer calls you could fit into a block. The number should at least
	// be a few hundred (250 at the time of writing but can change over time). Runs until panic.
	use node_primitives::Index;

	// execution ext.
	let mut t = new_test_ext(compact_code_unwrap(), false);
	// setup ext.
	let mut tt = new_test_ext(compact_code_unwrap(), false);

	let factor = 50;
	let mut time = 10;
	let mut nonce: Index = 0;
	let mut block_number = 1;
	let mut previous_hash: Hash = GENESIS_HASH.into();

	loop {
		let num_transfers = block_number * factor;
		let mut xts = (0..num_transfers).map(|i| CheckedExtrinsic {
			signed: Some((charlie(), signed_extra(nonce + i as Index, 0))),
			function: Call::Balances(pallet_balances::Call::transfer(bob().into(), 0)),
		}).collect::<Vec<CheckedExtrinsic>>();

		xts.insert(0, CheckedExtrinsic {
			signed: None,
			function: Call::Timestamp(pallet_timestamp::Call::set(time * 1000)),
		});

		// NOTE: this is super slow. Can probably be improved.
		let block = construct_block(
			&mut tt,
			block_number,
			previous_hash,
			xts
		);

		let len = block.0.len();
		print!(
			"++ Executing block with {} transfers. Block size = {} bytes / {} kb / {} mb",
			num_transfers,
			len,
			len / 1024,
			len / 1024 / 1024,
		);

		let r = executor_call::<NeverNativeValue, fn() -> _>(
			&mut t,
			"Core_execute_block",
			&block.0,
			true,
			None,
		).0;

		println!(" || Result = {:?}", r);
		assert!(r.is_ok());

		previous_hash = block.1;
		nonce += num_transfers;
		time += 10;
		block_number += 1;
	}
}

#[test]
#[should_panic]
#[cfg(feature = "stress-test")]
fn block_length_capacity_report() {
	// Just report how big a block can get. Executes until panic. Should be ignored unless if
	// manually inspected. The number should at least be a few megabytes (5 at the time of
	// writing but can change over time).
	use node_primitives::Index;

	// execution ext.
	let mut t = new_test_ext(compact_code_unwrap(), false);
	// setup ext.
	let mut tt = new_test_ext(compact_code_unwrap(), false);

	let factor = 256 * 1024;
	let mut time = 10;
	let mut nonce: Index = 0;
	let mut block_number = 1;
	let mut previous_hash: Hash = GENESIS_HASH.into();

	loop {
		// NOTE: this is super slow. Can probably be improved.
		let block = construct_block(
			&mut tt,
			block_number,
			previous_hash,
			vec![
				CheckedExtrinsic {
					signed: None,
					function: Call::Timestamp(pallet_timestamp::Call::set(time * 1000)),
				},
				CheckedExtrinsic {
					signed: Some((charlie(), signed_extra(nonce, 0))),
					function: Call::System(frame_system::Call::remark(vec![0u8; (block_number * factor) as usize])),
				},
			]
		);

		let len = block.0.len();
		print!(
			"++ Executing block with big remark. Block size = {} bytes / {} kb / {} mb",
			len,
			len / 1024,
			len / 1024 / 1024,
		);

		let r = executor_call::<NeverNativeValue, fn() -> _>(
			&mut t,
			"Core_execute_block",
			&block.0,
			true,
			None,
		).0;

		println!(" || Result = {:?}", r);
		assert!(r.is_ok());

		previous_hash = block.1;
		nonce += 1;
		time += 10;
		block_number += 1;
	}
}