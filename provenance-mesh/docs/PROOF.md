# Proof

Everything below is either verifiable on a public cluster or reproducible from
the bytes committed to this repository. Nothing here rests on a claim you have
to take on trust.

## 1. The baseline, reproduced against the code that publishes it

`account-cooker` [#2](https://github.com/solanabr/account-cooker/pull/2) contains
a test named `common_funder_limit_remains_measurable`, which asserts that the
funding channel's ROC AUC is exactly 1.000 — perfect operator attribution. It
passes:

```
$ cargo test -p cooker-eval common_funder_limit_remains_measurable
test experiment::tests::common_funder_limit_remains_measurable ... ok
```

That is the number this work exists to move, taken from the strongest
submission's own test rather than from a baseline we constructed.

`provenance-eval` reproduces it independently in
`a_star_funded_fleet_is_perfectly_attributable`, which asserts AUC within 1e-12
of 1.000 for a star-funded fleet.

## 2. Live on devnet

Program: [`8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39`](https://explorer.solana.com/address/8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39?cluster=devnet)

| scenario | outcome | compute units |
|---|---|---|
| full round settles — 8 distinct depositors, 8 payouts | PASS | 4,789 |
| settlement before deposits complete | refused, `0x6` `DepositsIncomplete` | 1,335 |
| round filled to capacity by one key | refused, `0x7` `AnonymitySetTooSmall` | 1,699 |
| settlement by a stranger | refused, `0xe` `UnauthorizedSettler` | 2,859 |

Round account [`EUNt2kmgqH9i4BAEforEqvGfnttMbZPZoeQ2jznBVAQt`](https://explorer.solana.com/address/EUNt2kmgqH9i4BAEforEqvGfnttMbZPZoeQ2jznBVAQt?cluster=devnet)

Settlement [`48mNCQbbgiET3KcqfHJx3HVYYH8dBppTf5V3JPRsCEvnPc5R91GV41QJHaGveadeRbwvPDjwZYYH1pPUAheHPuav`](https://explorer.solana.com/tx/48mNCQbbgiET3KcqfHJx3HVYYH8dBppTf5V3JPRsCEvnPc5R91GV41QJHaGveadeRbwvPDjwZYYH1pPUAheHPuav?cluster=devnet)
— executed in slot 478873841.

Every signature for all four scenarios is in [`PROOF-devnet.md`](PROOF-devnet.md).

### Value conservation, in on-chain state

After settling 8 payouts of 1,000,000 lamports each, the round account holds
**0.00844944 SOL** — exactly the rent-exempt minimum for its 1,086 bytes, and
nothing more. Every deposited lamport left the pool. The client independently
checks that all 8 recipients hold exactly one denomination before reporting PASS.

```bash
solana account EUNt2kmgqH9i4BAEforEqvGfnttMbZPZoeQ2jznBVAQt --url devnet
solana rent 1086 --url devnet    # 0.00844944 SOL
```

### A vulnerability this repository's own audit found, and the proof it is closed

Settlement originally verified only that its caller had *signed*. It never
compared the caller to anything, and the round stored no authority. Any stranger
could therefore call `settle` on a funded round, name eight addresses of their
own, and take every deposit. The accounting stayed balanced — value conserved —
but ownership did not.

This was not a reading of the code. It was demonstrated against the deployed
program on devnet:

- **Exploited**, on the vulnerable build:
  [`5WfMy39cNSP1xMJkSKefFe3T9v2gDrtLwjPSwNCiD7pMxksjmMSAgcD2fCXqX96Jco7RtXay6zk6pWPibF3L9iTs`](https://explorer.solana.com/tx/5WfMy39cNSP1xMJkSKefFe3T9v2gDrtLwjPSwNCiD7pMxksjmMSAgcD2fCXqX96Jco7RtXay6zk6pWPibF3L9iTs?cluster=devnet)
  — a stranger drains a round it never contributed to.
- **Refused**, on the fixed build: scenario 4 above, custom error `0xe`.

The fix stores the opening account as the round's `authority` and requires the
settler to match it. That trades "anyone can steal" for "the coordinator the
participants already chose could misdeliver" — the same class of assumption the
threat model already documents for relayers, and it is now written there
explicitly. Committing to a hash of the recipient set at open time would remove
even that, and is listed as future work rather than claimed.

Two regression tests lock it: `only_the_authority_can_settle` and
`a_depositor_is_not_automatically_a_settler`.

## 3. Real mainnet measurement

Two independent 300-slot windows, both committed:

| | window A | window B |
|---|---|---|
| slots returned | 300/300 | 300/300 |
| transfers extracted (top-level + CPI) | 60,268 | 72,080 |
| funded accounts | 20,171 | 22,186 |
| accounts at effective k = 1 | 90.8% | 86.4% |
| wallets under star-shaped funders | 14,342 | 12,673 |
| **their mean effective k** | **1.00** | **1.00** |
| pooled accounts that break the deposit-to-payout link | 34 of 42 | 35 of 40 |

**27,015 wallets at an effective anonymity set of exactly 1.00**, a figure
identical across both samples. The population shares differ (90.8% vs 86.4%) and
both are reported; picking the higher one would misrepresent a window as a
population.

```bash
cargo run -p provenance-mainnet -- report window-a
cargo run -p provenance-mainnet -- report window-b
```

Both reproduce from `data/window-a.json` and `data/window-b.json` in this
repository, not from live network state. Re-sampling is
`cargo run -p provenance-mainnet -- fetch 300 <name>`.

A 40-slot sample is kept at `data/window-40slot.json` for comparing window
*size* against window *position*: widening a window moves the population share
far less than moving it does.

**Measurement caveat.** A block window can only ever understate an account's
depositor count, so every anonymity figure is a lower bound. The breakage test
inherits that bias in one direction, which is why it is paired with the
recipient-signed test, decided inside a single transaction and immune to window
length.

## 4. Test suite

```
$ cargo test --workspace
provenance-core        9 passed
provenance-eval       37 passed
provenance-program    12 passed
provenance-mainnet     9 passed
                      -- 65 total, 0 failed
$ cargo clippy --workspace --all-targets -- -D warnings
clean, with pedantic lints enabled workspace-wide
```

Tests that encode the honest limits rather than the happy path:

- `a_lone_operator_cannot_reach_k_above_one` — pooling alone buys nothing
- `repeat_deposits_do_not_inflate_the_anonymity_set` — six deposits from one key
  is `k = 1`, and settlement is refused
- `pools_pay_out_exactly_what_they_took_in` — conservation in the simulator, so
  it models the program rather than a friendlier world
- `hop_chaining_defeats_the_shallow_attack_only` — the intuitive fix is not one
- `total_ties_score_one_half` — a defence that collapses every score must
  measure as chance, not as a perfect defence
- `only_the_authority_can_settle` — a stranger with a valid, complete round in
  front of them still cannot take it

## 5. Two bugs the live run caught that unit tests could not

Recorded because they are the argument for running against a real cluster. A
third — unauthorised settlement — is in section 2, and a unit test suite alone
would not have surfaced any of them.

**Borsh trailing bytes.** The round account is sized for a full 32-depositor
roster (1,054 bytes); the initial state encodes to 30. `try_from_slice` treats
the remaining zero bytes as corruption, so every deposit failed on chain with
`MalformedRoundState` while all 12 unit tests passed — they round-trip exact-size
buffers, which never exercises the mismatch. Fixed by deserializing from a
cursor.

**Rent exemption on depositors.** Depositors were funded with the denomination
plus a small margin, leaving them below the ~890,880-lamport rent-exempt floor
after depositing. Solana rejects any transaction that leaves a non-empty account
under that floor. Invisible to unit tests, which do not model rent.

## 6. A note on the test-harness gap

There is no in-process SVM harness (`litesvm`, `mollusk-svm`) in this workspace,
and that is not an omission. As of this writing the published crates do not
resolve: `solana-instruction` 3.4.0 requires `wincode` ^0.5 while the current
agave 4.1.x line pulls `solana-address` 2.7, which requires `wincode` 0.6. Any
graph enabling `solana-instruction`'s `wincode` feature — which both harnesses do
— fails to compile.

Verified independently against both crates. The workaround adopted here was to
drop the `solana-program` umbrella in favour of granular crates, which resolves
the conflict for the program and the client but cannot for an SVM harness. The
live-cluster proof in section 2 covers what such a harness would have.
