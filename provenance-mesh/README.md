# provenance-mesh

**The funding graph is the channel nobody closed.**

Every `account-cooker` submission so far measures behavioural noise — timing,
amounts, protocol mixes — and each one is careful and well-measured. None of
them touches how the agents were paid for, and all of them say so:

- `account-cooker` [#2](https://github.com/solanabr/account-cooker/pull/2) reports
  its own result plainly: *"the common-funder graph remains directly
  observable"*, and locks it into a test named
  `common_funder_limit_remains_measurable`, which asserts the funding channel's
  ROC AUC is exactly **1.000**. Its `PLAN.md` lists hiding a common funding
  source as an explicit non-goal.
- `account-cooker` [#1](https://github.com/solanabr/account-cooker/pull/1) drove
  clustering ARI from 0.4214 to 0.4140 across three honest attempts and reported
  each one, including the two that did nothing.
- `supersonic-tx` [#4](https://github.com/solanabr/supersonic-tx/pull/4) measures
  funding provenance at +0.27…+0.51 and leaves it open, noting it needs *"the
  same external crowd this tool specifies the interface for but does not itself
  supply."*
- `mirror-pool` [#1](https://github.com/solanabr/mirror-pool/pull/1) and
  [#2](https://github.com/solanabr/mirror-pool/pull/2) both show effective-k
  collapsing under funding-provenance partitioning.

Six strong submissions, one unanimous gap. This workspace closes it: it measures
the channel, shows the obvious fix does not work, supplies the crowd interface
`supersonic-tx` asked for, and enforces it on chain.

It is a layer, not a competitor. It sits underneath any of the three
architectures and does not ask a maintainer to choose against one.

---

## The finding, on real mainnet data

Two independent 300-slot mainnet windows, both committed to this repository. Every
number below reproduces from those bytes, not from live network state.

| | window A | window B |
|---|---|---|
| transfers | 60,268 | 72,080 |
| funded accounts | 20,171 | 22,186 |
| accounts at effective k = 1 | 90.8% | 86.4% |
| star-shaped funding clusters | 1,746 | 1,412 |
| wallets funded by a star | 14,342 | 12,673 |
| **mean effective k of those wallets** | **1.00** | **1.00** |
| pooled clusters | 142 | 87 |
| wallets funded by a pool | 2,379 | 3,210 |
| mean effective k of those wallets | 21.62 | 994.54 |

**27,015 wallets across the two windows sit at an effective provenance anonymity
set of exactly 1.00.** One identifiable origin, no ambiguity. That figure is
identical in both samples.

A star-shaped funder pays many accounts while being paid by almost none. The
largest observed paid 160 wallets in window A and 660 in window B, receiving from
nobody in either case.

The numbers that *do* move are the population shares — 90.8% against 86.4% — and
the anonymity that pools confer, which swung from 21.62 to 994.54 because window
B happened to contain a pool with 1,638 depositors. Both windows are reported
rather than the flattering one: what a window contains varies, the invariant does
not.

Note what is *not* claimed: nothing identifies these clusters as bot fleets,
airdrops, payroll, or anything else. Intent is not observable and none is imputed.
What is observable is the shape, and the shape decides the privacy.

Behavioural noise cannot move any of this. The funding edge is recorded before the
agent has behaved at all.

### Working provenance breaks already exist — and every one has a gatekeeper

This began as a check on an assumption, and the assumption was wrong.

A pool with a thousand depositors looks like it must confer anonymity. Often it
confers none, and the funding graph alone cannot tell you which: the answer is in
who *signed* the payout. A payout authorised by one of the pool's own depositors
publishes the link between a deposit and a destination. A payout authorised by
anyone else does not.

Testing every pooled account with 8 or more distinct depositors:

| verdict | window A | window B |
|---|---|---|
| breaks the deposit-to-payout link | **34** of 42 | **35** of 40 |
| passthrough — hides nothing | 4 | 2 |
| partial | 4 | 3 |

The largest pool in window B had **1,638 depositors and 1,929 payouts, none signed
by a depositor**.

That test is biased by window length — someone who deposited last week and
withdrew today reads as a stranger — so it is paired with one that is not: whether
the recipient signed its own payout, settled inside a single transaction and
immune to how far back the window reaches. Only 6 of 42 and 5 of 40 pools are
self-service; the largest sit at 0.00. The two measurements agree.

**So provenance privacy on Solana is not impossible. It is routine, and it is
locked behind an intermediary.** Every working pool here is custodial or mediated:
you obtain that anonymity by being someone's customer, not by choosing it.

That is the gap this workspace targets. Not the idea of a pool — a permissionless,
non-custodial one.

## The obvious fix does not work

The instinct is to insert intermediate hops so the funding trail is not a star.
Measured across 5 seeds on 16 operators × 8 agents:

| funding topology | mean achieved k | direct-funder | ancestor (d=8) | shared-ancestor | deposit-share |
|---|---|---|---|---|---|
| naive star | n/a | **1.000** | **1.000** | **1.000** | 0.500 |
| cascade, 3 hops | n/a | 0.500 | **1.000** | **1.000** | 0.500 |
| mesh round, 8 | 6.64 | 0.498 | 0.608 | 0.513 | 0.504 |
| mesh round, 16 | 10.93 | 0.496 | 0.527 | 0.500 | 0.497 |
| mesh round, 32 | 14.45 | 0.500 | 0.511 | 0.500 | 0.502 |
| mesh round, 64 | 15.90 | 0.500 | **0.500** | 0.500 | 0.500 |

ROC AUC 1.000 is perfect operator attribution; 0.500 is chance.

Cascading blinds a one-hop observer and looks like a fix. Against an observer
willing to walk the chain backwards it is worth **nothing** — still 1.000. That
is worse than doing nothing, because it buys confidence without buying privacy.

Pooled rounds drive the attack to chance, with a residual that decays as the
round widens.

## Attacker advantage is not anonymity

Reported because the first version of this analysis got it wrong. Holding 128
agents fixed and redistributing them across fewer operators:

| operators | nominal k | effective k | effective k (min) | ancestor AUC |
|---|---|---|---|---|
| 2 | 2.00 | 1.98 | 1.81 | 0.496 |
| 4 | 3.98 | 3.67 | 2.70 | 0.501 |
| 8 | 7.22 | 6.52 | 4.14 | 0.509 |
| 16 | 10.93 | 10.02 | 5.97 | 0.527 |
| 32 | 13.85 | 13.27 | **7.93** | 0.559 |

Read AUC alone and two operators look *safer* than thirty-two. They are not. AUC
measures whether an attacker beats the base rate; with two operators the
attacker needs no attack at all, because the prior already says "one of two".

Both numbers are therefore always reported together. Note the min-entropy column:
at 32 operators the nominal 13.85 is really **7.93** against a guessing
adversary. Your k is not your k.

## What runs on chain

A native Rust program (no Anchor). Several operators deposit an identical amount
into one pool; once full, the pool pays that amount out to a list of recipients.
The chain records who deposited and who was paid. It does not record — and
cannot reconstruct — which deposit paid for which recipient.

**Enforced, not documented:**

- uniform denominations, so payouts stay exchangeable
- a floor of *distinct* depositors before any payout
- conservation of value: a round pays out exactly what it took in
- fail-closed refusal: a round that cannot deliver its advertised anonymity
  aborts rather than settling with a silently weaker guarantee

### Live on devnet

Program [`8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39`](https://explorer.solana.com/address/8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39?cluster=devnet)

| scenario | result | compute units |
|---|---|---|
| full round settles — 8 distinct depositors, 8 payouts | PASS | 4,689 |
| settlement before the round fills | refused, `0x6` | 1,275 |
| round filled to capacity by one key | refused, `0x7` | 1,638 |

Every signature is in [`docs/PROOF-devnet.md`](docs/PROOF-devnet.md). After
settlement the round account holds exactly its rent and nothing more — value
conservation visible in on-chain state, not asserted in prose.

The negative cases carry the weight. Scenario 3 is the failure that would
otherwise be silent: a round that looks full but was filled by a single key
offers an anonymity set of one, and the program refuses it.

**Cost.** 4,689 CU to settle eight payouts. The Groth16 approaches in
`mirror-pool` report ~98k–108k CU to verify one membership proof. This is
roughly 20× cheaper, with no trusted setup and no ceremony — a different point
on the trade-off curve, not a replacement for them.

## Audit your own wallets

The measurement above describes Solana. This answers the question an operator
actually has:

```bash
cargo run -p provenance-mainnet -- scan my-fleet.txt
```

A newline-separated list of addresses you control. It reconstructs their funding
provenance from the chain, reports an effective anonymity set per wallet, and
tells you whether an observer can group them.

It needs no crowd, no protocol adoption, and no agreement with the rest of this
workspace. Pointed at eight wallets funded by one of the star-shaped funders
found above, it reaches the conclusion unaided:

```
| wallet             | funder             | funder's depositors | effective k |
| 28fSHE9ssvZs...    | 6uqxgxbsVJWL...    | 0                   | 1.00        |
| 28ftPmtJQpLc...    | 6uqxgxbsVJWL...    | 0                   | 1.00        |
...

| attack                | pairs scoring above zero | share |
| direct-funder-jaccard | 28 of 28                 | 100%  |
| ancestor-jaccard      | 28 of 28                 | 100%  |

**Linkable.** 100% of wallet pairs share a funding ancestor an observer can see.
These addresses read as one operator.
```

The report ends with what it cannot see — history beyond the most recent 100
transactions per address, ancestors more than two hops back, and everything off
chain. A clean result is evidence of nothing found, not proof of nothing there,
and the tool says so rather than leaving the reader to assume otherwise.

## Install

Requires **Rust 1.89 or newer** — `solana-pubkey` sets that floor, and an older
toolchain fails with an unhelpful error rather than a version message.

```bash
rustup toolchain install stable        # 1.89+
git clone <this repo> && cd provenance-mesh
cargo build --workspace
```

Everything except the on-chain program builds and runs with nothing else
installed. The measurement crates need no Solana toolchain and no cluster.

To build or deploy the program itself, add the Anza toolchain:

```bash
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"
cargo-build-sbf --manifest-path crates/provenance-program/Cargo.toml --arch v3
```

**`--arch v3` is required.** The runtime has disabled execution of earlier SBPF
versions, and the default (`v0`) deploys to an `invalid account data for
instruction` failure that does not name the cause.

## Reproduce

```bash
cargo test --workspace                                   # 74 tests
cargo clippy --workspace --all-targets -- -D warnings    # clean, pedantic

cargo run -p provenance-mainnet -- report window-a       # mainnet findings
cargo run -p provenance-mainnet -- report window-b
cargo run -p provenance-eval --example comparison        # topology table
cargo run -p provenance-eval --example crowd_size        # crowd-size table
```

Both mainnet windows are committed as JSON, so the analysis reproduces from the
bytes in this repository rather than from whatever mainnet looks like today.
Re-sampling is `cargo run -p provenance-mainnet -- fetch 300 <name>` and takes
roughly ten minutes against the public RPC, which rate-limits aggressively.

Running the live proof against a cluster needs a funded keypair:

```bash
cargo run -p provenance-e2e -- <PROGRAM_ID> https://api.devnet.solana.com <KEYPAIR>
```

## Layout

Design and rationale in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

| crate | role |
|---|---|
| `provenance-core` | funding-graph primitives: ancestors, funder sets, deposit multiplicity |
| `provenance-eval` | attacks, entropy metrics, synthetic topologies, star/pool classification, provenance-breakage testing |
| `provenance-program` | the on-chain round: native Rust, fail-closed |
| `provenance-mainnet` | mainnet sampler, real-data analysis, and the per-wallet audit |
| `provenance-e2e` | live-cluster proof, including the negative cases |

## What this does not do

See [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md) for the full treatment. In short:

- **It does not hide amounts.** Denominations are uniform and public.
- **It does nothing for a lone operator.** One operator filling its own rounds
  gets `k = 1`. This is locked in a test named
  `a_lone_operator_cannot_reach_k_above_one`, and enforced on chain by scenario 3.
- **The floor counts keys, not people.** One operator with `k` keys can fill a
  round and satisfy the floor while obtaining nothing. Sybil resistance needs a
  cost this protocol does not yet impose.
- **It is structural, not cryptographic.** There is no proof of unlinkability
  here, unlike the ZK constructions in `mirror-pool`.
- **It stops at the chain boundary.** Correlated IPs or RPC metadata defeat all
  of it.

## Licence

MIT, matching this repository's root licence. Every crate declares it in
`Cargo.toml`.
