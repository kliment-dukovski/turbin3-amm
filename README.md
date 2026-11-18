# AMM with Protocol Fee Treasuries

A Solana automated market maker built with Rust and Anchor for the Turbin3 Week 3 AMM assignment. Liquidity providers deposit two SPL tokens and receive LP tokens representing their share of the pool. Traders swap between the two tokens, paying a fee in the input token.

This project builds on the [ShrinathNR/amm_q2_26 starter](https://github.com/ShrinathNR/amm_q2_26). The assignment changes add separate X and Y treasury accounts, route swap fees to those accounts, and extend the swap tests with balance assertions in both directions.

## Instructions

| Instruction | Behavior |
| --- | --- |
| `initialize(seed, fee, authority)` | Creates the pool configuration, LP mint, two liquidity vaults, and two treasury token accounts. Stores the fee in basis points and an optional administrative authority. |
| `deposit(amount, max_x, max_y)` | Transfers X and Y into the pool and mints `amount` LP base units. The first deposit uses `max_x` and `max_y` to establish the reserves. Later deposits calculate the required tokens from the pool's existing liquidity. |
| `withdraw(amount, min_x, min_y)` | Burns LP tokens and returns the corresponding X and Y, subject to the caller's minimum amounts. |
| `swap(is_x, amount_in, min_amount_out)` | Collects the fee, deposits the remaining input into the pool, and transfers output to the trader subject to the minimum output. `true` selects X → Y, and `false` selects Y → X. |

## Accounts

| Account | Purpose and address |
| --- | --- |
| `config` | Stores the pool settings. PDA seeds are `b"config"` and the little-endian pool seed. Acts as token authority for the vaults and treasuries, and mint authority for the LP mint. |
| `mint_x`, `mint_y` | Existing SPL token mints identifying the pool's two assets. |
| `mint_lp` | LP mint with six decimals. PDA seeds are `b"lp"` and the config address. |
| `vault_x`, `vault_y` | Associated token accounts for the config PDA and each asset mint. Hold liquidity used for swaps and LP withdrawals. |
| `treasury_x` | Token account holding X fees. PDA seeds are `b"treasury_x"` and the config address. |
| `treasury_y` | Token account holding Y fees. PDA seeds are `b"treasury_y"` and the config address. |
| User token accounts | Hold the user's X, Y, and LP tokens. |

The initializer pays the account creation costs. Treasury accounts use their own PDA addresses so they remain separate from the liquidity vaults. Each treasury is constrained to the correct mint, config authority, and PDA seeds during swaps.

## Fee calculation

The fee uses basis points, where 10,000 basis points equals 100%. The swap handler requires the configured fee to be below 10,000.

```rust
let fee = (u128::from(amount) * u128::from(self.config.fee) / 10_000) as u64;
let net_input = amount - fee;
```

Integer division rounds the fee down to the nearest base unit. The multiplication uses `u128` to accommodate the intermediate result.

For a 10-token input with six decimals and a fee of 30 basis points (0.30%), the input is 10,000,000 base units. The treasury receives 30,000 base units (0.03 tokens), and the liquidity vault receives 9,970,000 base units (9.97 tokens).

For X → Y, both input transfers originate from `user_x`, with the fee going to `treasury_x`. For Y → X, they originate from `user_y`, with the fee going to `treasury_y`. The full configured swap fee goes to the protocol treasury.

The existing `constant-product-curve` library calculates output using the net input. Its fee argument is set to zero because the swap handler has already deducted the fee.

## Build and test

The local environment used Anchor CLI 1.2.1, Solana CLI 4.1.2, and the starter's Anchor dependencies at 1.0.1. The repository specifies Rust 1.89.0. The CLI and dependency versions produce a version warning, and the build and tests passed with the command below.

Run from the directory containing `Anchor.toml`.

```bash
ANCHOR_BUILD_SBF_ARCH=v0 anchor build --ignore-keys && \
cargo test --locked
```

The tests use LiteSVM and load `target/deploy/amm_video.so`. Build the program before running them. Explicitly selecting sBPF v0 resolved the local `InvalidAccountData` failure when LiteSVM loaded the program. `--ignore-keys` skips the deployment key consistency check for this local build.

To rerun both swap tests after changing only test code, use the following command.

```bash
cargo test --locked --test tests test_swap -- --nocapture
```

## Test coverage

| Test | Checks |
| --- | --- |
| `test_initialize` | Pool initialization succeeds with the treasury accounts included. |
| `test_deposit` | Initialization and a liquidity deposit succeed. |
| `test_withdraw` | Initialization, a deposit, and an LP withdrawal succeed. |
| `test_swap` | X → Y succeeds, X treasury receives 30,000 base units, X vault receives the net input, the user receives at least the requested Y, and the combined user/pool Y balance is conserved. |
| `test_swap_y_to_x` | The equivalent fee, net input, minimum output, and output token balance checks for Y → X. |

The completed local run reported five passing integration tests and one passing program ID unit test. The initialization, deposit, and withdrawal tests check transaction success. The swap tests also inspect token balances.

## Current scope

The program collects fees into treasury accounts controlled by the config PDA. A treasury withdrawal instruction is future work. The optional administrative authority is stored in the config, and administrative update and pool closure instructions are future work as well.

Pricing and liquidity calculations use the starter's curve library. Implementing a CPMM directly and writing a downtime mitigation discussion are the assignment's optional extensions.
