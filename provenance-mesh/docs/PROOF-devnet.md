# provenance-mesh live proof

- cluster: `https://api.devnet.solana.com`
- program: `8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39`
- payer: `6bdccwFmG2jcD4T9EsKti6N7Z84gbqjANbtetvfqg1BW`

## Scenario 1 - a full round settles

| step | detail | signature |
|---|---|---|
| open round | `82h7K8YHrfk8RehXLgKDbDTyvZq7trm8CBGnfhNg1rCA` | `2FYS4BJwPgk9TQjuxh5q865NrWJNaJsCNZ4uktEWduKToNYXLtBvvXfZcmhw8UDfnHBs1pGDhgbrZaZsRPWEnFes` |
| deposit 1/8 | `512yYquKRyVLYs4qxdd4Jjg5EehroLZKCGJCDd5qwrEe` | `N5kx2QWrWuscwqBcMCy12Ctt9tVYLFJz3rZnQvahmX9TY1bpPYGMZajCsCN5Ynx4owudvcuMRUnpx9pMtRcZyvp` |
| deposit 2/8 | `9poCx95ZKv1QDXnXwMGGqYZ87M6RF8jYsvouw4zUcF44` | `5xP9q1gRqx5w8nPrzFst9wyZQAejyLePQxcrtoUdyPcSXPjGVe9rgNvmeWmG7dBrMTgNFJzh6FAVfzR8czh2QDc3` |
| deposit 3/8 | `8kd2ncYbhCkPRZLvwqWCsRVnrLUyCfPeBEpfJENhCp76` | `26LYJQvgEC3K2cSSZR3oxfVnbUspS6Arkb8m6D16NWKM8aayjKiVqZpnVMzmrtq8HkQ3HzQwMa5Ep2He56AttpiP` |
| deposit 4/8 | `2EkHbqTP7LBtcF8xai8yGZM4RYGAdvH3aHgmBmuKtS8S` | `24JsFBj82mEQ5r96dgNJYL9KUTZ2MopQ5PZ6WPyUaMoDjK7d3j73qvwQr1cDLdmcrXZrRdqfkqCjyz6rKg1c6evN` |
| deposit 5/8 | `AnQcvRYdFMTsoygmD76NXHSQ2SL7RuWRzaLdET8yf2T4` | `3K9XRZebNKySd2dHKy4cGz6UqDCLyecAaCREeNY5cy6JH2Y4rJhccFdRHEAQzze1vJdb68NsWZ5iPFVKT8jZoPpN` |
| deposit 6/8 | `71d6vPd2tzveXEEjtNYXnhyEnf8oqbozG9YvTosh1RaH` | `3nzzWw1rZ63CrVtXESL4g8kEEksqr2PRFcVfa3RH3rYkoEuQEFKf2wChBLRqM4HVZVAJcG3PVKBbdBGgz3pHrdJM` |
| deposit 7/8 | `3oQAcNUqpMgJ4kfuGceXD8CjE1c84F3MAKELsYejohPv` | `2AFUEE8uSE6sGRPurA4nBdbBhFj7AdbaVyQeSiC9YDRLX3V1Ni4UY9mwBJAucksazyeUy4XtY1acWRumkF6fAoxv` |
| deposit 8/8 | `EcFCc6B2Fy2srVg3vbVEL1WWE7TQLLREutcXhbY3aGSF` | `4ZiS1sSHhD3a3beP9njdtULKm8Luzup11NtgujE2itTyYFyyc49rXC3D2MoQjDWKXfgNGB71ZuqBKR7q2XDtu95n` |
| settle 8 payouts | - | `2wLYhrZuxtgEnUAiUJC7FeAMqnaXdq9CSuXuaFHR7MdjaxR2GQo8jFdZMbbjQvYS9QuKXMDWQDpWr5bamYMvxJGM` |

All 8 recipients hold exactly 1000000 lamports; 8000000 lamports paid out against 8000000 deposited.

## Scenario 2 - settlement before the round fills is refused

Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0x6; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 2612 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0x6
`

## Scenario 3 - a round filled by one key is refused

Round filled to capacity by the single key `H4JA7cR1veucH4E8hyTCf2SWY5WCrE7fgyy9h2jWBKh`.
Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0x7; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 1746 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0x7
`

## Scenario 4 - a stranger cannot settle a funded round

Round is full and valid. Stranger `AFbcW2yQWwkmtp3341Q6NsiPybsHocFt1nT7XzyBFVGf` attempts to settle it to their own addresses.
Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0xe; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 2906 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0xe
`

## Scenario 5 - the authority cannot redirect the payout

Round committed to a recipient set and funded by 8 distinct depositors.
The authority itself now attempts to settle to a different set.
Refused on chain, as required: `RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: custom program error: 0xf; 3 log messages:
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 invoke [1]
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 consumed 3424 of 200000 compute units
  Program 8xrL8baL63gADaxWkDCWhnc8EceAKmq6oKmBtfmqSQ39 failed: custom program error: 0xf
`

The committed set settles normally: `LW8NgCRNsbk3KEgC1ek8HiSKmLbt3g2vLimPUPrhCFKRtLHW5w9J8PGZHdwf3Y9gfcf33u3xGrpkWXra5HHQUU9`

## Result

- PASS - full round settles
- PASS - early settlement refused
- PASS - single-depositor round refused
- PASS - unauthorized settler refused
- PASS - authority cannot redirect
