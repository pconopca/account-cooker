# Architecture

Five crates, layered so that the measurement never depends on the mechanism.
That separation is deliberate: the attacks and metrics must be usable to
evaluate *any* funding strategy, including ones that conclude this workspace's
own mechanism is not worth adopting.

```
provenance-core            graph primitives, no I/O, no Solana SDK
   |
   +-- provenance-eval     attacks, entropy metrics, topologies, classifiers
   |      |
   |      +-- provenance-mainnet    RPC sampler + real-data analysis
   |
   +-- provenance-program  the on-chain round (native Rust, no Anchor)
          |
          +-- provenance-e2e        live-cluster proof
```

## `provenance-core`

`FundingGraph` and the types it needs. Deliberately free of any RPC client or
Solana SDK dependency, so the same graph type serves synthetic fleets, mainnet
samples, and anything a third party wants to score.

Both directions are indexed at construction:

- `inbound: target -> source -> count` — counts rather than a set, because the
  *multiplicity* of deposits into a pool is itself an attack surface
- `outbound: source -> set of targets`

Attribution walks the graph backwards from tens of thousands of wallets. An
earlier version recomputed inbound counts per query, which is quadratic and made
a full mainnet report exceed two minutes; the eager index is what makes it
finish.

`FundingEdge` carries an optional `payer` — the fee payer of the transaction
that moved the value. Synthetic edges leave it `None`. It exists because whether
a pooled account genuinely breaks provenance is decided by who signed the payout,
which is not visible in the graph shape at all.

## `provenance-eval`

Four modules, each answering a different question.

**`attack`** — pairwise linkage scores an observer can compute from public data
alone. `DirectFunderJaccard` reproduces the exact signal `cooker-eval` reads as
`FeatureFamily::Funding`, so results are comparable rather than parallel.
`AncestorJaccard` walks backwards to depth *d*, which is what defeats hop
chaining. `SharedAncestorIndicator` is the cheap binary version a commodity
pipeline actually runs. `DepositShareAttack` computes the exact posterior that a
pool's public deposit counts permit.

No attack may read `EntityId`. Ground truth is applied only afterwards, to score
the attack — enforced by construction, since attacks receive only a graph and two
wallet ids.

**`metrics`** — `roc_auc` by the rank-sum identity with tie-averaged ranks, plus
Shannon and min-entropy and the effective-set-size conversion. Tie handling is
not a detail: a defence that collapses every score to one value must measure 0.5,
and a curve-integrating implementation can report 1.0 for that same input.

**`anonymity`** — absolute anonymity rather than attacker advantage. These are
different questions and reporting only the first is how the first version of this
analysis reached a wrong conclusion; both are now always reported together.

**`scenario`** — synthetic fleets under three funding topologies, holding agents
and owners constant so the only variable is how they were paid.

**`fleet`** and **`breakage`** — classifiers used against real data. `fleet`
separates a star from a pool by pairing out-degree with in-degree. `breakage`
answers whether a pool's payouts are authorised by its own depositors.

## `provenance-program`

A native Rust program. No Anchor: the state is one account and the logic is a few
hundred lines, so a framework would add dependency surface without removing work.

The umbrella `solana-program` crate is deliberately **not** used. It pulls
`solana-example-mocks`, which drags in a `wincode` major that conflicts with the
one the current agave line resolves to. Granular crates avoid it.

### Round lifecycle

```
open_round(nonce, commitment, denomination, k_min, capacity)
    -> creates the round PDA, seeds ["round", authority, nonce]
    -> validates before allocating, so a bad config costs the caller nothing

deposit  x capacity
    -> each depositor transfers exactly `denomination`
    -> records the depositor if new; refuses once full or once settling

settle(recipients[])            -- the whole set, in ascending order
    -> refuses unless the caller is the round's authority  (0xe)
    -> refuses unless deposits are complete                (0x6)
    -> refuses below the distinct-depositor floor          (0x7)
    -> refuses a set that does not hash to the commitment  (0xf)
    -> refuses out-of-order or duplicate recipients, rent breach
    -> pays `denomination` to each recipient
```

Settlement is all-or-nothing. The commitment covers the whole recipient set, so
a partial payout would let the authority reveal a prefix and abandon the rest;
`capacity` recipients must be presented in one transaction, which caps a round
at roughly sixty.

Ascending order is required rather than merely checked for duplicates. That
gives the set one canonical encoding, so the commitment is unambiguous, and it
rules out duplicates in the same pass.

Both checks exist because an audit found the pool takeable twice over: first by
any signer, then by its own authority. See `PROOF.md`.

### Why the mapping is absent rather than hidden

Recipients arrive at settlement as a flat list. The program has no field
associating a deposit with a payout, and no instruction that could produce one.
That is the whole mechanism: the link is not encrypted or proven-away, it was
never recorded.

The cost of that choice is that the guarantee is structural, not cryptographic —
there is no unlinkability proof, unlike the Groth16 constructions in
`mirror-pool`. The benefit is roughly 20× less compute and no trusted setup.

### Account layout

`Round` is borsh-encoded into a fixed 1,118-byte account sized for a full
32-depositor roster. Since the encoded state is usually shorter, reads use
`BorshDeserialize::deserialize` from a cursor rather than `try_from_slice`, which
rejects trailing bytes and caused every on-chain deposit to fail until it was
found by a live run.

## `provenance-mainnet`

Fetches `getBlock` with `jsonParsed` and extracts every system-program
instruction that moves lamports — `transfer`, `transferWithSeed`,
`createAccount`, `createAccountWithSeed`, `withdrawNonceAccount` — from both
top-level and inner instructions, recording each one's fee payer. Failed
transactions, self-transfers and zero-lamport creations are excluded.

The instruction list is a table rather than a match arm because an earlier
version matched only the two `transfer` variants and silently dropped roughly a
third of the graph. Account creation is not an edge case here: it is how a fresh
wallet comes into existence.

Samples are written gzipped to `data/<name>.json.gz` and committed, so the
analysis reproduces from bytes in the repository rather than from live network
state. They compress to roughly a fifth of their size, which is the difference
between a repository someone will clone and one they will not.

It also carries `scan`, which works the other way around: given a list of
addresses, it fetches each one's recent transactions, then does the same for
every account that funded them. Two hops, because the question is not the full
ancestry of the money — it is whether the wallet's immediate funder was itself
paid by a crowd. Transactions are requested in JSON-RPC batches to keep
round-trips down, since the public endpoint rate-limits hard enough that a
naive per-signature loop does not finish.

Its window measurements are bounded by the window: an account funded before it
appears unfunded, and a busy account's depositor set is truncated. That bias runs
one way, so every figure is a lower bound. Two independent windows are sampled
rather than one, because window *position* moves the population shares
considerably more than window *size* does.

What it cannot see at all is a program moving lamports by direct mutation, which
produces no instruction to parse. That includes this workspace's own `settle`:
the devnet proof transactions contain no parsed system instruction, so a fleet
funded through provenance-mesh would be invisible to provenance-mainnet.
Attributing raw balance deltas means guessing which decrease paid which increase,
ambiguous as soon as more than one account moves, so the gap is documented rather
than papered over with a heuristic. See `THREAT_MODEL.md`.

## `provenance-e2e`

Drives a deployed program through three scenarios against a live cluster: a full
round settling, and two refusals. The refusals are the point — they are what
separates an enforced guarantee from a documented intention.

Uses the 3.x Solana client line, which resolves cleanly alongside the program's
granular dependencies where an in-process SVM harness currently does not.
