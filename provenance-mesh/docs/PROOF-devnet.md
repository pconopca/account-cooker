# provenance-mesh live proof

- cluster: `https://api.devnet.solana.com`
- program: `8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39`
- payer: `6bdccwFmG2jcD4T9EsKti6N7Z84gbqjANbtetvfqg1BW`

## Scenario 1 - a full round settles

| step | detail | signature |
|---|---|---|
| open round | `4Qu2xRJYycE54XKr9iTdABRQqkHP48MCedkWaCSFD55D` | `3bSNdSA8VVvJ6aRnS8izeSWKGNiXrTU8ixuqrFfxaLCkMUSg3tyg5n5t9JbE4ikLFh95tsQYquN5tL13MJnUmX9C` |
| deposit 1/8 | `3tAJZ7D1CjCYTvkG9bTxJhK385JdFe72YNT9qTfyVHkf` | `2Bjmo64kdP7FvNR5Q7EBhRBcB5webGk3fDbTbiuBD2tnDpURq6RLNMQ84xDhyfFQWkmwF7tvdoz8sTakZh2kWPcf` |
| deposit 2/8 | `64abmfWU9reCL2qReX79Q7KVv8kPXNucQtazdx5xFMEB` | `62dELoudUkVnmsi8NQvvi2PqBV3REiPGDdhUFXpqPKmu3bRhNGBWRbfp1qGQLyVXhAenegsb4rf94qp6NPtJjFaK` |
| deposit 3/8 | `35AcLRtDLvtDmm1gFNHMUgZdRDZpHVZJCiEN9ExBDMJ4` | `5AL9qwuviGHvHMqQtCkW28XXdceGBGaan8pgw6rSqHdjWSpGSqNHEN3uQYsHBEDkXkSMguJbMARxEbsRAaz6tR9m` |
| deposit 4/8 | `5CWuMwekoz7qMiVw4Q6tEtSdQJogETDedGAPj7syLYGC` | `KXxxdZTTZWv9GVRNU2K8im8EF8dMPECfZKpHfwvFBRUyTY43en3DohiG2KeWeTvE32RM1wbLW9N1AQXE22Pt5Wr` |
| deposit 5/8 | `GCe5E9sPp1cQUkxcrmr6ontdLFy5UvhxiwXyauqzYz6C` | `zJkTZu92mcp996kxJ8nVcD4ZfDgv32b6CRD4KTm5ogSJsRe6XWSzuf3vbRT2aNzy9Am4nbFdg4mjC4wEJdgfZ4u` |
| deposit 6/8 | `DFpaE9LkTpuPDNzFD468SU12QUxC1394vZt56brJrQGw` | `dMUKsZ5poWVwsUEkBGGFFhMatPzRjfGzTPBpJRCrVbDmfVXA7KK7RVYTWtoXNMFjBT5zgAwSnrwKYi5FLvXcPMj` |
| deposit 7/8 | `92x1ksQ5ZoJ8eChG4rGQWCsw9wx9mKmh7rw2qF6vBHXi` | `54Wn8yfEJFQwm3QoFdjtivtVjDDWJ9wwSJnYo1sYbxrQER5sQk6Uxj7qZhXYZoeWcD2Ek1SWneToj937yaP2iyar` |
| deposit 8/8 | `2PpXM1MPLyp1kZx9XY7AKhuJvK9PaukPqhyYa2fYP3C6` | `nsLnjJL5VK9wazvDrRyxL9TsJMTavTCjNrYPyoRSwLTdGfhfXSwKDsy5xkJJzSLGXX1Ku2bYMxJ15Rvo7cXuv6K` |
| settle 8 payouts | - | `2NuhF9GkEoB68vWWcDWPURxPVJkYsQpko3WVU54XCxLFGXKTm5XYyNUnHDnMkkWRMbkShViY1HyaxpoKaqg5fva8` |

All 8 recipients hold exactly 1000000 lamports; 8000000 lamports paid out against 8000000 deposited.

## Scenario 2 - settlement before the round fills is refused

Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0x6; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 1275 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0x6
`

## Scenario 3 - a round filled by one key is refused

Round filled to capacity by the single key `CDCLVXXG8LrpUUs8CHCvYgMxWzgTLHJnCLrszbcz1huk`.
Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0x7; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 1638 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0x7
`

## Result

- PASS - full round settles
- PASS - early settlement refused
- PASS - single-depositor round refused
