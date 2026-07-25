# provenance-mesh live proof

- cluster: `http://127.0.0.1:8899`
- program: `8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39`
- payer: `6bdccwFmG2jcD4T9EsKti6N7Z84gbqjANbtetvfqg1BW`

## Scenario 1 - a full round settles

| step | detail | signature |
|---|---|---|
| open round | `4gtkFgwGjZ4u2H6WnVzEhnBxvD67SqLJxc35ivw2oPim` | `2Y9d2ZbGknMgJnCzPQ43arD93rcmnXtbw1UCx4ABAzU9LRu2NLTKgaAQXzGoxcRFWR7cVikDby48QCWeDh7pRzoi` |
| deposit 1/8 | `2CYVKjaCm1xxxvCaD8xQDG8rGf2A14DvRtL3jfg9Ugji` | `58sZmxAQQTtCqXs5MvPUW2eL3KtZ2MNw6kkpZtQgSa3JK1GzhdWuop8Gurumh3P1cdcsmSL82jeB9YHrUSBCvHfs` |
| deposit 2/8 | `GBLb4cdocqCWWe7GnXFzyD1H2pPi8tkZPqXXXpmQ8K1Q` | `T2uDnnvqQ1KRcT6ZG3fKorhXJ7zLLLxpGkum1DnsoQMKZH6aQVtUfDa2KhknH3FWXs9iq2szrHVATpCYwCPLZi9` |
| deposit 3/8 | `CgX1ShSNrGCz1W9q4QjkWCf5m8ECga6GyC5B9v1o4PMZ` | `2PJzza8ELT3x5jewemqESQtgpyG86USfq9s8URLsx51kBPco3H2gaxCZSjxAL5JHJE4FLLVZ4Vj6Wz5UQZ4YqfDe` |
| deposit 4/8 | `AZNDiP3yV8mm9pWg7ru1h444Fw2DyvUuiVD6FHpa8eAH` | `35W7C6dDoa2zU96vM8dEPjmFhYrBEGMYyijubs9sKv6CqD2T9nTCeXQw89WszbYXWtEBjUp1eAZu8NjGz6h4MrK6` |
| deposit 5/8 | `3TGrzbttfqnQdq7zoFtNmT2KQpEPKq3sSEj747XFcFJN` | `4oghtkJJm15zUw2CK2uwjUVUyU2dFmiMvc58grs6276xWfoo2EaGzNiPAPr75WnXzYNxvmVGYxpJGbtwPZwka6fS` |
| deposit 6/8 | `HQVHAmdLnYgRAiC9esXmsRNreFvcPZQu4ztARcKF9RgJ` | `c2qR7WYNTrP8EsmQHBRsBmhxWtr2hgy1m6Voqin23isJkMDSJU5UjDvP3mpNniGfQTNbEUGoxJ58kDgNtpf7yrZ` |
| deposit 7/8 | `2sEgwtvjsnD35SHcrhsx6s6SFqWvASHADDco8YmTURPK` | `42K6oiArjs9HZXT2Tqycp2n24Qh6sswxUefLj1d7uE4Vq37xPSYMcEopjdn9FPpXQ7WJNBRDs8dQw47g8xmpYmwL` |
| deposit 8/8 | `AGCsUTpJfMsZH37NcCi8aVbvPSQwmCHEAKHcCegUs2zs` | `2caL19kK2JK76tTuQTFuwugvndUaCSGrXJNyjuavJhY4QCScTXPmLnwR1CxBzpDgDZo1j9tekq9c8kvaM46oHJsD` |
| settle 8 payouts | - | `5VgJjtGa43nTaQhpkDSdAMkUJ8a2zVheu9mrnzAMdh29xCFaqq1gStVcpFUvkRsjtgSBeEU2CBHoDDeZ4fZgcvy4` |

All 8 recipients hold exactly 1000000 lamports; 8000000 lamports paid out against 8000000 deposited.

## Scenario 2 - settlement before the round fills is refused

Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0x6; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 1275 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0x6
`

## Scenario 3 - a round filled by one key is refused

Round filled to capacity by the single key `B8XWYcLbtudQcc2SXiyC1pSNAtza99agHBc7mNN6QQff`.
Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0x7; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 1638 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0x7
`

## Result

- PASS - full round settles
- PASS - early settlement refused
- PASS - single-depositor round refused
