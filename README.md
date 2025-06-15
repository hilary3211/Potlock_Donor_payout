#  POTLOCK Donor Payout  Smart Contract

Built on the NEAR blockchain, facilitates a donation and airdrop system for campaigns. It maintains detailed records of user donations and airdrop distributions, enabling donors to receive token or NFT rewards post-donation. The contract integrates with a Potlock NFT contract ```potlock-nfts.testnet``` to mint NFTs as rewards for eligible donors.


## Features
- Donation Tracking: Records user donations in yoctoNEAR, associating them with a donor’s account ID and campaign.

- Airdrop Management: Logs airdrop distributions (tokens or NFTs) with details like recipient, amount, timestamp, campaign ID, and reward type.

- NFT Rewards: Allows donors to claim NFT rewards via a cross-contract call to a Genadrop NFT contract, updating airdrop records with minted NFT details.

- Campaign Support: Tracks donations and airdrops per campaign, with pagination for retrieving airdrop records.

- Storage Management: Requires deposits to cover storage costs for state updates, ensuring scalability on NEAR.



## Technologies Used
- Rust: The programming language used to write the smart contract.

- NEAR SDK: For interacting with the NEAR blockchain.

- NEAR Testnet: The contract is deployed on the NEAR Testnet for testing.


## Contract ID
```
potlock-donor2.testnet
```


## How to Build Locally?

Install [`cargo-near`](https://github.com/near/cargo-near) and run:

```bash
npm install -g near-cli
```

```bash
cargo near
```

```bash
git clone https://github.com/hilary3211/Potlock_Donor_payout.git
```

```bash
cd Potlock_Donor_payout
```

```bash
cargo near build
```

## How to Test Locally?

```bash
cargo test
```


## How to Deploy?

Deployment is automated with GitHub Actions CI/CD pipeline.
To deploy manually, install [`cargo-near`](https://github.com/near/cargo-near) and run:

```bash
cargo near deploy build-reproducible-wasm <account-id>
```

## Useful Links

- [cargo-near](https://github.com/near/cargo-near) - NEAR smart contract development toolkit for Rust
- [near CLI](https://near.cli.rs) - Interact with NEAR blockchain from command line
- [NEAR Rust SDK Documentation](https://docs.near.org/sdk/rust/introduction)
- [NEAR Documentation](https://docs.near.org)
- [NEAR StackOverflow](https://stackoverflow.com/questions/tagged/nearprotocol)
- [NEAR Discord](https://near.chat)
- [NEAR Telegram Developers Community Group](https://t.me/neardev)
- NEAR DevHub: [Telegram](https://t.me/neardevhub), [Twitter](https://twitter.com/neardevhub)
