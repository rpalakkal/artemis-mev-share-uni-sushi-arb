# Artemis MEV-Share Uniswap/Sushiswap Arbitrage

This project implements a simple probabilistic UniswapV2/Sushiswap arbitrage on MEV-Share, using the Artemis MEV-Share template found [here](https://github.com/FrankieIsLost/artemis-mev-share-template).

## Strategy

### Sync

We first load all WETH pools that exist on both Uniswap V2 and Sushiswap for which WETH is `token1` in the pair. The list of these pools was found [here](https://github.com/paradigmxyz/artemis/blob/main/crates/strategies/mev-share-uni-arb/resources/uni_sushi_weth_pools.csv).

### Processing

After loading in the pools, we listen to MEV-Share events for transactions that are made to any of the pools (Uniswap and Sushiswap) loaded in. After detecting such a transaction, we find its corresponding pair (either the Uniswap V2 pair of the Sushiswap pair) and submit backruns of various size.

## Directory Structure

The project is structured as a mixed Rust workspace with a Foundry project under
`contracts/` and typesafe auto-generated bindings to the contracts under
`bindings/`.

```
├── Cargo.toml
├── bot // <-- Your bot logic
├── contracts // <- The smart contracts + tests using Foundry
├── bindings // <-- Generated bindings to the smart contracts' abis (like Typechain)
```

## Testing

Given the repository contains both Solidity and Rust code, there's 2 different
workflows.

### Solidity

Forge is using submodules to manage dependencies. Initialize the dependencies:

```bash
forge install
```

If you are in the root directory of the project, run:

```bash
forge test --root ./contracts
```

If you are in in `contracts/`:

```bash
forge test
```

### Rust

```
cargo test
```

## Generating Rust bindings to the contracts

Rust bindings to the contracts can be generated via `forge bind`, which requires
first building your contracts:

```
forge bind --bindings-path ./bindings --root ./contracts --crate-name bindings
```

Any follow-on calls to `forge bind` will check that the generated bindings match
the ones under the build files. If you want to re-generate your bindings, pass
the `--overwrite` flag to your `forge bind` command.

## Installing Foundry

First run the command below to get `foundryup`, the Foundry toolchain installer:

```sh
curl -L https://foundry.paradigm.xyz | bash
```

Then, in a new terminal session or after reloading your `PATH`, run it to get
the latest `forge` and `cast` binaries:

```sh
foundryup
```

For more, see the official
[docs](https://github.com/gakonst/foundry#installation).
