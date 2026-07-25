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
open_round(nonce, denomination, k_min, capacity)
    -> creates the round PDA, seeds ["round", authority, nonce]
    -> validates before allocating, so a bad config costs the caller nothing

deposit  x capacity
    -> each depositor transfers exactly `denomination`
    -> records the depositor if new; refuses once full or once settling

settle(recipients[])
    -> refuses unless deposits are complete           (0x6)
    -> refuses below the distinct-depositor floor     (0x7)
    -> refuses duplicates, self-payment, rent breach
    -> pays `denomination` to each recipient
```

Settlement may be batched across several calls; `settled_count` is capped at
`capacity`, so a round can never pay out more than it took in.

### Why the mapping is absent rather than hidden

Recipients arrive at settlement as a flat list. The program has no field
associating a deposit with a payout, and no instruction that could produce one.
That is the whole mechanism: the link is not encrypted or proven-away, it was
never recorded.

The cost of that choice is that the guarantee is structural, not cryptographic —
there is no unlinkability proof, unlike the Groth16 constructions in
`mirror-pool`. The benefit is roughly 20× less compute and no trusted setup.

### Account layout

`Round` is borsh-encoded into a fixed 1,054-byte account sized for a full
32-depositor roster. Since the encoded state is usually shorter, reads use
`BorshDeserialize::deserialize` from a cursor rather than `try_from_slice`, which
rejects trailing bytes and caused every on-chain deposit to fail until it was
found by a live run.

## `provenance-mainnet`

Fetches `getBlock` with `jsonParsed`, extracts system-program transfers from both
top-level and inner instructions, and records each transfer's fee payer. Failed
transactions and self-transfers are excluded.

Samples are written to `data/<name>.json` and committed, so the analysis
reproduces from bytes in the repository rather than from live network state.

It also carries `scan`, which works the other way around: given a list of
addresses, it fetches each one's recent transactions, then does the same for
every account that funded them. Two hops, because the question is not the full
ancestry of the money — it is whether the wallet's immediate funder was itself
paid by a crowd. Transactions are requested in JSON-RPC batches to keep
round-trips down, since the public endpoint rate-limits hard enough that a
naive per-signature loop does not finish.

Its window measurements are bounded by the window: an account funded before the window
appears unfunded, and a busy account's depositor set is truncated. The bias is
one-directional and every figure is therefore a lower bound. Two independent
windows are sampled rather than one, because window *position* moves the
population shares considerably more than window *size* does.

## `provenance-e2e`

Drives a deployed program through three scenarios against a live cluster: a full
round settling, and two refusals. The refusals are the point — they are what
separates an enforced guarantee from a documented intention.

Uses the 3.x Solana client line, which resolves cleanly alongside the program's
granular dependencies where an in-process SVM harness currently does not.
