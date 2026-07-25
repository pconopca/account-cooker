# Threat model

## The adversary

A passive observer with the complete chain history and unlimited compute. It
sees every transfer, every account, every slot, forever. It does not need to
break cryptography, run a validator, or observe the network layer — everything
below assumes it reads only what the ledger publishes.

This is not a hypothetical adversary. It is what a commodity analytics pipeline
already is.

## What it is trying to do

Partition a set of wallets into the entities that control them. Behavioural
tooling fights this on timing, amounts, and protocol mixes. This workspace
concerns a different signal: **who paid for the wallet**, which is a hard,
permanent, first-order fact rather than a statistical tendency.

## What the program enforces

These are checked on chain and proven by the negative cases in
[`PROOF-devnet.md`](PROOF-devnet.md):

| invariant | mechanism | proof |
|---|---|---|
| Payouts are exchangeable | one uniform denomination per round | `settle` pays `denomination` to every recipient |
| The set is full before it leaks | payouts refused until deposits complete | scenario 2, error `0x6` |
| The set is real | payouts refused below the distinct-depositor floor | scenario 3, error `0x7` |
| No value is created | the round settles its full capacity exactly once, or not at all | `authorize_payouts`, and the round account holding exactly rent afterwards |
| The set cannot be diluted after the fact | deposits refused once settlement starts | `RoundSettling` |
| Only the coordinator may settle | settler must equal the round's authority | scenario 4, error `0xe` |
| The coordinator cannot choose where the money goes | recipient set committed before the first deposit | scenario 5, error `0xf` |

The design rule throughout is **fail closed**. A round that cannot deliver the
anonymity it advertised aborts. It never settles with a quietly weaker guarantee,
because an operator who is not told has no way to compensate.

## What is measured and open

### Deposit multiplicity — open, currently quiet

A round hides which payout a deposit paid for. It does not hide **how many**
deposits each funder made, and those counts are public. A funder that deposited
three times into a pool that made eight payouts has told the observer that three
of those eight are its own.

`DepositShareAttack` implements the exact posterior this permits:
`Σ nᵢ(nᵢ−1) / N(N−1)`, the probability two distinct payouts from the pool share
a funder. Measured across the topology sweep, it sits at **0.494–0.504** — no
pairwise advantage at the tested parameters, because within a round every pair
receives the same score and the channel only discriminates across rounds.

It is reported as open rather than solved. It would bite whenever one operator
dominates a round, and nothing in the protocol currently prevents that beyond
the distinct-depositor floor.

### Residual ancestor overlap — open, decays with round size

Two agents of the same operator still share that operator's source wallet
further back in the graph. `AncestorJaccard` measures the residual at **0.608**
for 8-payout rounds, falling to **0.500** at 64. Wider rounds dilute the shared
ancestor among more candidates.

### Settlement delivery — closed, after two attempts

The round's authority coordinates settlement but chooses nothing. The recipient
set is committed as a hash before the first deposit is accepted, and settlement
recomputes that hash over the accounts presented. A depositor can check the
commitment against the list it was promised before paying in, rather than hoping
afterwards.

This took two rounds of fixing, and both are recorded in [`PROOF.md`](PROOF.md)
rather than quietly amended. Settlement originally checked only that its caller
had *signed*, so any stranger could drain a funded round — demonstrated on
devnet. Requiring the authority closed that and left a worse hole: the authority
itself could settle to its own addresses, which would have made this pool
custodial in exactly the way it criticises working mainnet pools for being. The
commitment closes both.

What remains: the authority still decides *whether* to settle. It cannot
misdirect the funds, but it can decline to act, stranding deposits in the round.
There is no timeout or refund path, and that is a real gap rather than a
deliberate choice.

### Sybil funders — open, not addressed

The floor counts distinct **keys**, not distinct people. One operator with `k`
keys can fill a round alone, satisfy the floor, and obtain no privacy at all
while the program reports a healthy `k`.

`achieved_k` is therefore an upper bound on real anonymity, never a measurement
of it. The `mirror-pool` submissions price this with an entry fee; that
mitigation is compatible with this design and is not implemented here.

### The lone operator — a structural floor, not a bug

A single operator pooling with itself gets an effective `k` of exactly 1. This
is not a limitation to be engineered away; it is what anonymity sets *are*.
Pooling requires a crowd. What the protocol can do is refuse to pretend when one
has not formed, and it does.

Locked in `a_lone_operator_cannot_reach_k_above_one`, and enforced on chain by
scenario 3.

## Out of scope entirely

- **Amounts.** Denominations are uniform and public. This layer says nothing
  about how much value moved.
- **Cryptographic unlinkability.** There is no zero-knowledge proof here. The
  unlinkability is structural — the mapping is absent from the ledger rather
  than hidden behind a hardness assumption. A ZK construction gives a stronger
  guarantee at roughly 20× the compute cost and the price of a trusted setup.
- **The network layer.** Correlated source IPs, RPC provider metadata, or
  submission timing defeat every guarantee above. Nothing on chain can help.
- **Timing of deposits.** Deposit slots are public and this design does not
  randomise or batch them. An operator that deposits for all its agents in one
  burst has said so.
- **Downstream behaviour.** Once an agent is funded, what it does is the domain
  of `account-cooker`. This layer only concerns how it came to hold lamports.

## Measurement caveats

The mainnet figures come from contiguous block windows. An account funded before
the window appears to have no funder; a busy account's depositor set is truncated
to that window. That bias is one-directional — it can only understate depositor
counts — so every reported anonymity figure is a lower bound.

**A second limit runs the other way, and it is the sharper one.** The extractor
reads parsed system-program instructions. A program that owns an account can move
its lamports directly, producing no instruction to parse, and those transfers are
invisible here. The measurement therefore describes wallets funded *by system
instructions*, not all funded wallets.

The direction matters. Program-mediated payouts are disproportionately the
pooled kind — the very provenance breaks this work is about — so missing them
understates how much pooling exists. And a wallet paid by both a system transfer
and a program payout reads as single-funder when it is not, which inflates the
headline share rather than deflating it.

This is not hypothetical: **the settlement transactions produced by this
repository's own program are invisible to its own extractor.** Lamports move by
direct mutation inside `settle`, so the devnet proof in [`PROOF.md`](PROOF.md)
contains no parsed system instruction at all. A fleet funded through
provenance-mesh would not be detected by provenance-mainnet.

Attributing direct lamport mutations means reading pre/post balances and
guessing which decrease paid which increase, which is ambiguous whenever more
than one account moves. Rather than ship a heuristic that cannot be validated,
the gap is stated.

**Funding in SPL tokens is out of scope and equally invisible.** The graph is
denominated in lamports. An operator that funds a fleet in USDC and never sends
it SOL leaves nothing this tool reads. That is a deliberate boundary rather than
an oversight — provenance through token accounts is a different graph, with its
own mint authorities and associated-account derivation — but it means a clean
result here says nothing about token-funded wallets.

Two independent 300-slot windows are reported rather than one. The population
share differs between them (85.4% and 88.4% at effective k = 1; 71.9% and 79.9%
on the structural measure), so no single window should be read as a population
statistic. What is stable across both is the invariant: wallets a star-shaped
funder solely paid sit at an effective k of 1.00 in each. That is what the
argument rests on.

The synthetic topologies were written by the same author as the attacks. That
circularity is why the mainnet measurement exists; the synthetic tables should
be read as mechanism illustrations, and the mainnet numbers as the evidence.

## Abuse

Pooled funding is dual-use. The same mechanism that stops a trading desk being
front-run also makes sybil fleets harder to attribute — including fleets built
to farm airdrops or manufacture volume.

Two things are worth stating plainly. First, this design deliberately does
**not** mix value: denominations are uniform and conserved, and no participant's
balance changes as a result of another's. The pool holds funds only between
deposit and settlement, pays out exactly what it took in, and pays only to a set
fixed before the first deposit. It breaks the *linkage* between funder and
recipient, not the traceability of funds in aggregate.

Second, the on-chain record of who deposited into which round is permanent and
public. Association-set proofs and viewing-key disclosure, as implemented in the
`mirror-pool` submissions, compose with this design and would give a participant
a way to demonstrate the provenance of their own payout without revealing the
mapping for anyone else. That is the right place for compliance to live, and it
is not built here.

## What would close the remaining gaps

In rough order of value:

1. **A refund path**, so deposits are recoverable if the authority never
   settles. The largest remaining gap.
2. **A cost on entry**, to price sybil funders.
3. **Deposit-count normalisation**: require every funder in a round to deposit
   the same number of times, closing the multiplicity channel by construction
   rather than measuring it quiet.
4. **Deposit timing jitter**, so burst deposits stop announcing fleet size.
5. **Multi-round layering**, to attack the residual ancestor overlap that a
   single round leaves at small `k`.
