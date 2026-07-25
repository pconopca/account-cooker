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
| full round settles — 8 distinct depositors, 8 payouts | PASS | 4,906 |
| settlement before deposits complete | refused, `0x6` `DepositsIncomplete` | 1,746 |
| round filled to capacity by one key | refused, `0x7` `AnonymitySetTooSmall` | 2,612 |
| settlement by a stranger | refused, `0xe` `UnauthorizedSettler` | 2,906 |
| the authority redirecting the payout | refused, `0xf` `RecipientSetMismatch` | 3,424 |

Round account [`82h7K8YHrfk8RehXLgKDbDTyvZq7trm8CBGnfhNg1rCA`](https://explorer.solana.com/address/82h7K8YHrfk8RehXLgKDbDTyvZq7trm8CBGnfhNg1rCA?cluster=devnet)

Settlement [`2wLYhrZuxtgEnUAiUJC7FeAMqnaXdq9CSuXuaFHR7MdjaxR2GQo8jFdZMbbjQvYS9QuKXMDWQDpWr5bamYMvxJGM`](https://explorer.solana.com/tx/2wLYhrZuxtgEnUAiUJC7FeAMqnaXdq9CSuXuaFHR7MdjaxR2GQo8jFdZMbbjQvYS9QuKXMDWQDpWr5bamYMvxJGM?cluster=devnet)
— executed in slot 478875561.

Every signature for all five scenarios is in [`PROOF-devnet.md`](PROOF-devnet.md).

### Value conservation, in on-chain state

After settling 8 payouts of 1,000,000 lamports each, the round account holds
**0.00867216 SOL** — exactly the rent-exempt minimum for its 1,118 bytes, and
nothing more. Every deposited lamport left the pool. The client independently
checks that all 8 recipients hold exactly one denomination before reporting PASS.

```bash
solana account 82h7K8YHrfk8RehXLgKDbDTyvZq7trm8CBGnfhNg1rCA --url devnet
solana rent 1118 --url devnet    # 0.00867216 SOL
```

### Two ways to take the pool, found by this repository's own audit

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

The first fix stored the opening account as the round's `authority` and required
the settler to match. That closed the stranger's path — and left a worse one.

**The authority could still settle to itself.** Nothing bound the recipients to
anything: a coordinator could open a round, wait for `k` strangers to deposit,
and pay the whole pool to addresses it controlled. That would have made this
pool custodial in precisely the way it criticises the working pools on mainnet
for being, which is not a bug in the code so much as a contradiction of the
claim the repository is built on.

The recipient set is now committed at open time — a hash of the set in strictly
ascending order, fixed before the first deposit is accepted. Settlement
recomputes it over the accounts actually presented and refuses anything else, so
the authority coordinates the round and decides nothing. A depositor can check
the published commitment against the list it was promised *before* paying in.

- **Refused**, authority substituting recipients: scenario 5 above, `0xf`.
- **Accepted**, the committed set, in the same round:
  [`LW8NgCRNsbk3KEgC1ek8HiSKmLbt3g2vLimPUPrhCFKRtLHW5w9J8PGZHdwf3Y9gfcf33u3xGrpkWXra5HHQUU9`](https://explorer.solana.com/tx/LW8NgCRNsbk3KEgC1ek8HiSKmLbt3g2vLimPUPrhCFKRtLHW5w9J8PGZHdwf3Y9gfcf33u3xGrpkWXra5HHQUU9?cluster=devnet)

Because the commitment covers the whole set, settlement is all-or-nothing: a
partial payout would let the authority reveal a prefix and abandon the rest.
That caps a round at roughly sixty recipients, which is the cost of the fix.

Four regression tests lock it: `only_the_authority_can_settle`,
`a_depositor_is_not_automatically_a_settler`, `settlement_is_all_or_nothing`,
and `commitment_requires_ascending_order_and_rejects_duplicates`.

## 3. Real mainnet measurement

Two independent 300-slot windows, both committed:

| | window A | window B |
|---|---|---|
| lamport-moving system instructions | 107,037 | 106,087 |
| funded accounts | 32,594 | 39,228 |
| exactly one funder, itself paid by nobody | 23,427 (71.9%) | 31,343 (79.9%) |
| effective anonymity set of 1 | 85.4% | 88.4% |
| wallets solely funded by a star | 9,259 | 20,445 |
| **their mean effective k** | **1.00** | **1.00** |
| wallets solely funded by a pool | 2,779 | 2,838 |
| their mean effective k | 94.44 | 64.72 |
| pooled accounts that break the deposit-to-payout link | 34 of 50 | 22 of 35 |

**54,770 accounts across both windows were paid by exactly one account that was
itself paid by nobody.** That figure needs no entropy model: it is read directly
off the graph.

```bash
cargo run -p provenance-mainnet -- report window-a
cargo run -p provenance-mainnet -- report window-b
```

Both reproduce from `data/window-a.json.gz` and `data/window-b.json.gz` in this
repository. Re-sampling is `cargo run -p provenance-mainnet -- fetch 300 <name>`.

### Three measurement errors this audit found

**The extractor was reading two thirds of the graph.** It matched only
`transfer` and `transferWithSeed`, and ignored `createAccount` — which moves
lamports into a brand-new account, and is therefore how a fresh wallet comes into
existence. Fresh wallets are precisely what a fleet consists of. Measured over
six mainnet blocks: 854 lamport-moving system instructions ignored against 1,310
captured. `createAccountWithSeed` and `withdrawNonceAccount` were missing for the
same reason. Covered by `account_creation_is_a_funding_edge`,
`seeded_creation_and_nonce_withdrawal_are_edges_too` and
`a_zero_lamport_creation_funds_nothing`.

**The anonymity measure inflated its own headline.** `provenance_posterior`
returned a point mass for any wallet with more than one funder — the strongest
possible claim about roughly 5% of accounts, while the comment directly above it
said it made none. That manufactured the invariant an earlier version led with:
"every wallet funded by a star sits at 1.00" held only because multi-funder
wallets had been forced there. Restricted to wallets a star *solely* funded, it
survives honestly. Covered by
`several_funders_widen_the_candidate_set_rather_than_collapsing_it` and
`a_multi_funder_wallet_inherits_every_crowd_it_was_paid_from`.

**The breakage test read only the fee payer.** Roughly 6% of mainnet
transactions carry more than one signer. A depositor that authorised its own
withdrawal while a relayer covered the fee therefore counted as an unrelated
third party — inflating the break rate, which is the number supporting the claim
that pools genuinely break provenance. Reading every signer moved window B from
31 of 40 pools to 22 of 35, and more than doubled the passthroughs. Covered by
`a_depositor_who_signs_but_does_not_pay_the_fee_still_counts`.

All three moved the numbers in the direction that made the work look worse, and
none touched the invariant.

**Measurement caveats.** A block window can only ever understate an account's
depositor count, so every anonymity figure is a lower bound.

Separately, the extractor reads parsed system instructions and cannot see a
program moving lamports by direct mutation. That includes this program's own
settlements — the transactions in section 2 contain no parsed system instruction
— so a fleet funded through provenance-mesh would be invisible to
provenance-mainnet. Since program-mediated payouts skew pooled, this understates
how much pooling exists, and a wallet paid by both routes reads as single-funder
when it is not. Funding denominated in SPL tokens rather than SOL is out of
scope entirely and equally invisible. Full treatment in
[`THREAT_MODEL.md`](THREAT_MODEL.md).

## 4. Test suite

```
$ cargo test --workspace
provenance-core        9 passed
provenance-eval       47 passed
provenance-program    16 passed
provenance-mainnet    12 passed
                      -- 84 total, 0 failed
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
- `account_creation_is_a_funding_edge` — the omission that had the extractor
  reading two thirds of the graph
- `several_funders_widen_the_candidate_set_rather_than_collapsing_it` — the
  measure must not claim certainty it does not have

## 5. Two bugs the live run caught that unit tests could not

Recorded because they are the argument for running against a real cluster. The
two takeover paths in section 2 are a third and a fourth, and a unit suite alone
would not have surfaced any of them.

**Borsh trailing bytes.** The round account is sized for a full depositor roster
(1,054 bytes at the time; 1,118 now); the initial state encodes to 30. `try_from_slice` treats
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
