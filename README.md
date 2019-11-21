# stafi-node

[Stafi](https://stafi.io) is:
- A Decentralize Protocol to Provide the liquidity of Your Staking Assets

STAFI Protocol solves the contradiction between the token liquidity and Mainnet security by issuing ABS tokens, which provides the liquidity of your Staking Assets. ABS token increases the staking rate to a higher level (100%, theoretically) ,and it could be tradable, its security is guided by STAFI Protocol which ensure ABS token is the only collateral that can apply to redeem staking asstes from original staking blockchain ( Tezos, Cosmos, Polkadot, etc,.).

## Start
If your device is clean (such as a fresh cloud VM) you can use this script, otherwise, proceed with the *Initial Setup*.
```
./setup.sh
```
To create a keypair, install subkey with `cargo install --force --git https://github.com/paritytech/substrate subkey`. Then run the following:
```
subkey generate
```
To create an ED25519 keypair, run the following:
```
subkey -e generate
```
To create derived keypairs, use the mnemonic generated from a method above and run:
```
subkey inspect "<mnemonic>"//<derive_path>
```
For example:
```
subkey inspect "rare sweet witness roof pig hill exhibit only sentence mail south fan"//stash
```
Then proceed to the *Running* instructions or follow the instructions below for the manual setup.

### Initial Setup

```
curl https://sh.rustup.rs -sSf | sh
rustup update stable
rustup update nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
cargo install --git https://github.com/alexcrichton/wasm-gc
```

You will also need to install the following packages:

Linux:
```
sudo apt install cmake pkg-config libssl-dev git clang libclang-dev
```

Mac:
```
brew install cmake pkg-config openssl git llvm
```

### Building

```
cargo build --release
```

### Running

Ensure you have a fresh start if updating from another version:
```
./scripts/purge-chain.sh
```
To start up the Stafi node and connect to the latest testnet, run:
```
./target/release/stafi --chain=stafi --name <INSERT_NAME>
```

## Implemented Modules

### Stafi

* [Multisig](https://github.com/stafiprotocol/stafi/tree/master/node/modules/stafi-multisig)
* [Tokenbalances](https://github.com/stafiprotocol/stafi/tree/master/node/modules/tokenbalances)
