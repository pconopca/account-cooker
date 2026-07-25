# provenance-mesh

**The funding graph is the channel nobody closed.**

Every `account-cooker` submission so far measures behavioural noise — timing,
amounts, protocol mixes — and each one is careful and well-measured. None
addresses how the agents were paid for, and several say so outright:

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

Five submissions, one open channel. This workspace measures it, shows the
obvious fix does not work, and ships a permissionless mechanism that closes it
on chain.

It is a funding layer, not a competing cooker. Nothing here conflicts with the
architectures in #1, #2 or #3 — but it is not integrated with them either, and
that integration is not written yet.

---

## The finding, on real mainnet data

Two independent 300-slot mainnet windows, both committed to this repository.
Every number below reproduces from those bytes, not from live network state.

| | window A | window B |
|---|---|---|
| transfers | 117,265 | 120,021 |
| funded accounts | 42,883 | 35,190 |
| **exactly one funder, itself paid by nobody** | **31,251 (72.9%)** | **25,956 (73.8%)** |
| effective anonymity set of 1 | 88.8% | 87.5% |
| star-shaped funding clusters | 2,410 | 2,613 |
| wallets *solely* funded by a star | 14,011 | 11,400 |
| **their mean effective k** | **1.00** | **1.00** |
| pooled clusters | 138 | 164 |
| wallets solely funded by a pool | 2,617 | 2,492 |
| their mean effective k | 45.07 | 119.62 |

**57,207 accounts across the two windows were paid by exactly one account that
was itself paid by nobody.** 72.9% and 73.8% — a single identifiable origin,
established by inspection rather than inferred from a model.

The entropy-based reading agrees: 88.8% and 87.5% sit at an effective anonymity
set of 1 once every funder's crowd is accounted for.

A star-shaped funder pays many accounts while being paid by almost none. Wallets
whose *only* funder is such an account measure at exactly 1.00 in both windows —
25,411 of them. Wallets whose only funder is a pooled account measure at 45.07
and 119.62.

That contrast is the whole finding. Two accounts can fund the same number of
wallets and hand them anonymity sets two orders of magnitude apart, decided
entirely by whether the funder was itself paid by a crowd.

Behavioural noise cannot move any of this. The funding edge is recorded before
the agent has behaved at all.

Note what is *not* claimed: nothing identifies these clusters as bot fleets,
airdrops, payroll, or anything else. Intent is not observable and none is
imputed. What is observable is the shape, and the shape decides the privacy.

**Two corrections behind these numbers**, both found by auditing this repository
and both documented rather than quietly applied.

The extractor originally read only `transfer` and `transferWithSeed`, and missed
`createAccount` entirely — which is how a fresh wallet comes into existence, and
fresh wallets are exactly what a fleet is made of. Measured over six mainnet
blocks, that was 854 lamport-moving instructions ignored against 1,310 captured.
Re-sampling with the gap closed roughly doubled the graph, from 60,268 and
72,080 edges to 117,265 and 120,021.

The anonymity measure separately collapsed every multi-funder wallet to a point
mass while its own comment claimed it made no assertion about them. That
inflated the population share and manufactured a cleaner invariant than the data
supported. Each funder now contributes its own candidate set.

Both corrections moved the numbers and neither weakened the conclusion: window
agreement on the structural measure tightened from 70.8/71.8 to 72.9/73.8.

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
| breaks the deposit-to-payout link | **33** of 43 | **31** of 40 |
| passthrough — hides nothing | 3 | 2 |
| partial | 7 | 7 |

The largest pools run to well over a thousand depositors with no payout signed
by any of them.

That test is biased by window length — someone who deposited last week and
withdrew today reads as a stranger — so it is paired with one that is not: whether
the recipient signed its own payout, settled inside a single transaction and
immune to how far back the window reaches. Only a handful of pools in each
window are self-service, and the largest sit at 0.00. The two measurements
agree.

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
- **the recipient set, committed before the first deposit lands** — the
  authority coordinates the round but has no discretion over where the money
  goes, and a depositor can verify the commitment before paying in
- fail-closed refusal: a round that cannot deliver its advertised anonymity
  aborts rather than settling with a silently weaker guarantee

### Live on devnet

Program [`8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39`](https://explorer.solana.com/address/8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39?cluster=devnet)

| scenario | result | compute units |
|---|---|---|
| full round settles — 8 distinct depositors, 8 payouts | PASS | 4,906 |
| settlement before the round fills | refused, `0x6` | 1,746 |
| round filled to capacity by one key | refused, `0x7` | 2,612 |
| a stranger tries to settle a funded round | refused, `0xe` | 2,906 |
| the authority tries to redirect the payout | refused, `0xf` | 3,424 |

Every signature is in [`docs/PROOF-devnet.md`](docs/PROOF-devnet.md). After
settlement the round account holds exactly its rent and nothing more — value
conservation visible in on-chain state, not asserted in prose.

The negative cases carry the weight. Scenario 3 is the failure that would
otherwise be silent: a round that looks full but was filled by a single key
offers an anonymity set of one, and the program refuses it.

Scenarios 4 and 5 exist because an audit of this workspace found two ways to
take the pool. Settlement originally checked only that the caller had *signed*,
so any stranger could drain a funded round — demonstrated on devnet, then fixed.
Requiring the round's authority closed that but left the authority itself able
to settle to addresses of its own, which would have made this pool custodial in
exactly the way the pools it criticises are. The recipient set is now committed
before the first deposit, and both attacks are refused on chain. Every
transaction, including the successful exploit against the vulnerable build, is
in [`docs/PROOF.md`](docs/PROOF.md).

**Cost.** 4,906 CU to settle eight payouts. The Groth16 approaches in
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
workspace. Pointed at eight mainnet wallets that one of the star-shaped funders
above solely paid, it reaches the conclusion unaided — the run below is verbatim
output from 25 July 2026, abbreviated in the middle:

```
| wallet                                       | funder                                       | funder's depositors | effective k |
| 24eHXEdjtHnbcV7eC37FHCYntZLgBo2mc8ADmDg9TLr6 | 6uqxgxbsVJWLWJfKipEJ5n21Jq51nYba9aVSjnEdXSPy | 0                   | 1.00        |
| 28ftPmtJQpLcDqZCszW3vyizAuVjjZYanprTFTzSBEYX | 6uqxgxbsVJWLWJfKipEJ5n21Jq51nYba9aVSjnEdXSPy | 0                   | 1.00        |
| ... six more, identical                      |                                              |                     |             |

**Mean effective anonymity set: 1.00** (min-entropy 1.00)

| attack                | pairs scoring above zero | share |
| direct-funder-jaccard | 28 of 28                 | 100%  |
| ancestor-jaccard      | 28 of 28                 | 100%  |

**Linkable.** 100% of wallet pairs share a funding ancestor an observer can see.
These addresses read as one operator.
```

It reads live chain data, so a run today will differ as those accounts keep
transacting. The verdict for this particular set will not: their funder has no
depositors, and nothing about that can change retroactively.

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
cargo test --workspace                                   # 83 tests
cargo clippy --workspace --all-targets -- -D warnings    # clean, pedantic

cargo run -p provenance-mainnet -- report window-a       # mainnet findings
cargo run -p provenance-mainnet -- report window-b
cargo run -p provenance-eval --example comparison        # topology table
cargo run -p provenance-eval --example crowd_size        # crowd-size table
```

Both mainnet windows are committed as gzipped JSON, so the analysis reproduces
from the bytes in this repository rather than from whatever mainnet looks like today.
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
- **Settlement is all-or-nothing.** The commitment covers the whole recipient
  set, so the set has to be presented in one transaction. That caps a round at
  roughly sixty recipients.
- **It stops at the chain boundary.** Correlated IPs or RPC metadata defeat all
  of it.
- **The measurement cannot see program-mediated transfers.** It reads parsed
  system instructions; a program moving lamports by direct mutation produces
  none. That includes this workspace's own settlement, so a fleet funded through
  provenance-mesh would be invisible to provenance-mainnet. The gap and its
  direction are in [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md).

## Licence

MIT, matching this repository's root licence. Every crate declares it in
`Cargo.toml`.
