# provenance-mesh live proof

- cluster: `https://api.devnet.solana.com`
- program: `8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39`
- payer: `6bdccwFmG2jcD4T9EsKti6N7Z84gbqjANbtetvfqg1BW`

## Scenario 1 - a full round settles

| step | detail | signature |
|---|---|---|
| open round | `EUNt2kmgqH9i4BAEforEqvGfnttMbZPZoeQ2jznBVAQt` | `2JkQTRL9vRSEyZ82aoajF8eT5FQimBJwd1XQFreywGAppMXriYpm7H94wthZ6sfdYNRxBsXRex1uth8jCARPTReT` |
| deposit 1/8 | `EoeABUYTyhtLsvhfkBGNnmh4PvFfxTWExKrTa2thkUck` | `4TzUKVaxKptkD4CV78dF1yFfUrwLXGEBJYknLVXD1jBdEQyBfisUatirJhsMm1dwJzSPsdtQxzcDKFTtFjt6iYRf` |
| deposit 2/8 | `5p3xWSjteggw5jq3sgnYtR1rTx7DRwQctetLzRyYfD7R` | `4uKdt3ESCrGdhRK77dtqVhPnzjTmmDVweSXQpkpitaJ5dCdZVKBGgzduBuP4KQWHSrHy6PWaWXbVrJ59byQNHDAZ` |
| deposit 3/8 | `23D4WjXdwjwGVteTA6c41zdGYRNKLAbMaah6eAwhQYES` | `4EQghtyeWuC3zvGMkJKhEzz3SyaAcLJ613YQXZ9JaS5uvzAP1KYFzwXFUY7R8X3QDzTU6dezpcGKcwQ2vyBX1htt` |
| deposit 4/8 | `4JigRVKUK45M2XFw32FhAogDiZxH7zTNEEzRaZRvn4yN` | `5g98fMA1muBfsotEJRZBtTPvTPcrQWAqVaE7fNqY7dhL4tysKBDcvwmk2LjCTo1UuaT1uEwpD7kVKiFW4jgHTNtN` |
| deposit 5/8 | `9wcavuQ5QmNm7PpdCQbGLZVJritg28A2cmUNCpR83Ls4` | `37T2W5xHYJimfEG8Ft1M2GS5MHFQ9PjGCw3fJP3yTdfdAcj3E12HdbkxwafCg8JHNiXN4CTmH6qJUsXie23cPN7M` |
| deposit 6/8 | `He1U8ZZyw1WHJ51gkA3qYC3NtUr46agbmQ5VAYsFHJx5` | `zMB8Evcnq5BE4axwcmZxEUjAnKcYQUcCnwYsJ8GHdhqXRudxpXPMJ4HutGFCX5imJWcfqz2vJoqQSxs4B1UrEkT` |
| deposit 7/8 | `4nREUvYf2rkMDectH9rVZBQhB3uj6JNCgaJuYjcnn3eW` | `DF8FxjtKA45vNE7A8U3MWyBFDW71HZfGZ1qNhWhsTUH11kyWg9ufyuTrqTxTsRpqzoQAZymJgZPMZ1VFPqcDfvZ` |
| deposit 8/8 | `HKbQUz4uXLT4QzRBxYfUJVBvB8u3CcQipysLpzhYgSmm` | `3YYTa6iZbw54agfRFUu4jiwk5tYopMHdx5yibinywCiG4P3AadyY6W2jSaY8NJQ1poAAYMLqEHEB9tnbyRM6EKfK` |
| settle 8 payouts | - | `48mNCQbbgiET3KcqfHJx3HVYYH8dBppTf5V3JPRsCEvnPc5R91GV41QJHaGveadeRbwvPDjwZYYH1pPUAheHPuav` |

All 8 recipients hold exactly 1000000 lamports; 8000000 lamports paid out against 8000000 deposited.

## Scenario 2 - settlement before the round fills is refused

Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0x6; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 1335 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0x6
`

## Scenario 3 - a round filled by one key is refused

Round filled to capacity by the single key `6WN5tfsufhq57DDmz9qfVtjK6cZeAadcJvL6wbwz5GFa`.
Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0x7; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 1699 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0x7
`

## Scenario 4 - a stranger cannot settle a funded round

Round is full and valid. Stranger `HT9MwTqmGHDiMdyPJCRK6BQbCafVJSYx5Msmv2aj2KRt` attempts to settle it to their own addresses.
Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0xe; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 2859 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0xe
`

## Result

- PASS - full round settles
- PASS - early settlement refused
- PASS - single-depositor round refused
- PASS - unauthorized settler refused
