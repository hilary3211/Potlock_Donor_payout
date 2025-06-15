use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{UnorderedMap, Vector};
use near_sdk::json_types::U128;
use near_sdk::{env, log, near_bindgen, AccountId, Promise, Gas, NearToken};
use near_sdk::serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use schemars::JsonSchema;
use near_sdk::PromiseResult;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct DonorPayouts {
    donors: UnorderedMap<AccountId, Donor>,
    airdrop_records: Vector<AirdropRecord>,
    total_distributed: u128,
    admin: AccountId,
    potlock_nfts_contract: AccountId,
    token_contract: AccountId, 
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub enum DonationType {
    #[schemars(with = "String")] Pot { pot_id: AccountId },
    Campaign { campaign_id: String },
    Direct,
    Project { project_id: String },
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, JsonSchema, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct AirdropRecord {
    #[schemars(with = "String")]
    pub recipient: AccountId,
    #[schemars(with = "String")]
    pub amount: U128,
    pub timestamp: u64,
    pub paid: bool,
    pub reward_type: RewardType,
    pub donation_type: DonationType,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub enum RewardType {
    Token,
    NFT { channel_id: String, token_id: String },
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, JsonSchema, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Donor {
    #[schemars(with = "String")]
    pub wallet_id: AccountId,
    #[schemars(with = "String")]
    pub donation_amount: U128,
    #[schemars(with = "String")]
    pub airdrop_amount: U128,
    pub paid: bool,
    pub reward_types: Vec<RewardType>,
    pub donation_types: Vec<DonationType>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct PaginatedAirdropRecords {
    pub records: Vec<AirdropRecord>,
    pub has_more: bool,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct PaginatedDonors {
    pub donors: Vec<Donor>,
    pub has_more: bool,
}

impl Default for DonorPayouts {
    fn default() -> Self {
        Self {
            donors: UnorderedMap::new(b"d"),
            airdrop_records: Vector::new(b"a"),
            total_distributed: 0,
            admin: env::predecessor_account_id(),
            potlock_nfts_contract: "potlock-nfts.testnet".parse().unwrap(),
            token_contract: "token.testnet".parse().unwrap(),
        }
    }
}

#[near_bindgen]
impl DonorPayouts {
    #[init]
    pub fn new(potlock_nfts_contract: Option<AccountId>, token_contract: Option<AccountId>) -> Self {
        let admin = env::predecessor_account_id();
        Self {
            donors: UnorderedMap::new(b"d"),
            airdrop_records: Vector::new(b"a"),
            total_distributed: 0,
            admin,
            potlock_nfts_contract: potlock_nfts_contract.unwrap_or("potlock-nfts.testnet".parse().unwrap()),
            token_contract: token_contract.unwrap_or("token.testnet".parse().unwrap()),
        }
    }

    fn assert_admin(&self) {
        assert_eq!(env::predecessor_account_id(), self.admin, "Only admin can call this function");
    }

    #[payable]
    pub fn log_airdrop(&mut self, recipient: AccountId, channel_id: String, donation_type: DonationType, amount: U128) {
        self.assert_admin();
        let amount_u128: u128 = amount.into();
        let attached_amount = env::attached_deposit().as_yoctonear();
        match &donation_type {
            DonationType::Campaign { campaign_id } => assert!(campaign_id.len() <= 64, "Campaign ID must be 64 characters or less"),
            DonationType::Project { project_id } => assert!(!project_id.is_empty(), "Project ID must not be empty"),
            DonationType::Pot { pot_id } => assert!(env::is_valid_account_id(pot_id.as_bytes()), "Invalid pot_id"),
            DonationType::Direct => (),
        }

        let reward_type = if channel_id.is_empty() {
            RewardType::Token
        } else {
            RewardType::NFT {
                channel_id,
                token_id: String::new(),
            }
        };

        let record = AirdropRecord {
            recipient: recipient.clone(),
            amount,
            timestamp: env::block_timestamp(),
            paid: false,
            reward_type: reward_type.clone(),
            donation_type: donation_type.clone(),
        };
        self.airdrop_records.push(&record);

        let mut donor = self.donors.get(&recipient).unwrap_or(Donor {
            wallet_id: recipient.clone(),
            donation_amount: U128(0),
            airdrop_amount: U128(0),
            paid: false,
            reward_types: vec![],
            donation_types: vec![],
        });

        donor.airdrop_amount = U128(donor.airdrop_amount.0 + amount_u128);
        donor.donation_amount = U128(donor.donation_amount.0 + attached_amount);

        // Add donation_type if not already present
        if !donor.donation_types.contains(&donation_type) {
            donor.donation_types.push(donation_type.clone());
        }

        // Add reward_type if not already present
        if !donor.reward_types.contains(&reward_type) {
            donor.reward_types.push(reward_type.clone());
        }

        self.donors.insert(&recipient, &donor);
        self.total_distributed += amount_u128;

        log!("Logged airdrop for {}: {} tokens, donation_type {:?}", recipient, amount_u128, donation_type);
    }

    #[payable]
    pub fn record_donation(&mut self, donation_type: DonationType) {
        let signer = env::predecessor_account_id();
        let attached_amount = env::attached_deposit().as_yoctonear();
        assert!(attached_amount > 0, "Attached deposit must be greater than 0");
        match &donation_type {
            DonationType::Campaign { campaign_id } => assert!(campaign_id.len() <= 64, "Campaign ID must be 64 characters or less"),
            DonationType::Project { project_id } => assert!(!project_id.is_empty(), "Project ID must not be empty"),
            DonationType::Pot { pot_id } => assert!(env::is_valid_account_id(pot_id.as_bytes()), "Invalid pot_id"),
            DonationType::Direct => (),
        }

        let mut donor = self.donors.get(&signer).unwrap_or(Donor {
            wallet_id: signer.clone(),
            donation_amount: U128(0),
            airdrop_amount: U128(0),
            paid: false,
            reward_types: vec![],
            donation_types: vec![],
        });

        donor.donation_amount = U128(donor.donation_amount.0 + attached_amount);

        // Add donation_type if not already present
        if !donor.donation_types.contains(&donation_type) {
            donor.donation_types.push(donation_type.clone());
        }

        self.donors.insert(&signer, &donor);
        log!("Recorded donation of {} yoctoNEAR for {}, donation_type {:?}", attached_amount, signer, donation_type);
    }

    #[payable]
    pub fn send_nft_reward(&mut self) -> Promise {
        let signer = env::predecessor_account_id();
        let donor = self.donors.get(&signer).expect("Donor not found");
        assert!(!donor.paid, "Payout already completed");

        let channel_id = donor
            .reward_types
            .iter()
            .find_map(|r| match r {
                RewardType::NFT { channel_id, .. } => Some(channel_id.clone()),
                _ => None,
            })
            .expect("No NFT reward type found for donor");

        log!("Initiating NFT mint for {}", signer);

        Promise::new(self.potlock_nfts_contract.clone())
            .function_call(
                "nft_mint".to_string(),
                json!({
                    "receiver_id": signer,
                    "channel_id": channel_id,
                    "proof": None::<String>,
                })
                .to_string()
                .into_bytes(),
                env::attached_deposit(),
                Gas::from_tgas(120)
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(10))
                    .on_nft_mint_callback(signer)
            )
    }

    // #[payable]
    // pub fn send_token_reward(&mut self) -> Promise {
    //     let signer = env::predecessor_account_id();
    //     let donor = self.donors.get(&signer).expect("Donor not found");
    //     assert!(!donor.paid, "Payout already completed");
    //     assert!(
    //         donor.reward_types.contains(&RewardType::Token),
    //         "Donor reward type does not include Token"
    //     );
    //     assert!(donor.airdrop_amount.0 > 0, "No tokens to payout");

    //     log!("Initiating token transfer of {} for {}", donor.airdrop_amount.0, signer);

    //     Promise::new(self.token_contract.clone())
    //         .function_call(
    //             "ft_transfer".to_string(),
    //             json!({
    //                 "receiver_id": signer,
    //                 "amount": donor.airdrop_amount,
    //             })
    //             .to_string()
    //             .into_bytes(),
    //             NearToken::from_yoctonear(1), // Standard 1 yoctoNEAR for FT transfer
    //             Gas::from_tgas(50)
    //         )
    //         .then(
    //             Self::ext(env::current_account_id())
    //                 .with_static_gas(Gas::from_tgas(10))
    //                 .on_token_transfer_callback(signer, donor.airdrop_amount)
    //         )
    // }



    #[payable]
    pub fn send_token_reward(&mut self) -> Promise {
        let signer = env::predecessor_account_id();
        let donor = self.donors.get(&signer).expect("Donor not found");
        assert!(!donor.paid, "Payout already completed");
        assert!(
            donor.reward_types.contains(&RewardType::Token),
            "Donor reward type does not include Token"
        );
        assert!(donor.airdrop_amount.0 > 0, "No tokens to payout");

        log!("Initiating token reward process for {}", signer);

        Promise::new(self.token_contract.clone())
            .function_call(
                "storage_balance_of".to_string(),
                json!({ "account_id": signer })
                    .to_string()
                    .into_bytes(),
                NearToken::from_yoctonear(0),
                Gas::from_tgas(20),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(80))
                    .on_storage_check_callback(
                        signer.clone(),
                        donor.airdrop_amount,
                        env::attached_deposit(),
                    ),
            )
    }

    #[private]
    pub fn on_storage_check_callback(
        &mut self,
        signer: AccountId,
        amount: U128,
        attached_deposit: NearToken,
    ) -> Promise {
        assert_eq!(
            env::promise_results_count(),
            1,
            "Expected one promise result"
        );

        match env::promise_result(0) {
            PromiseResult::Successful(result) => {
                let balance: Value = serde_json::from_slice(&result)
                    .expect("Failed to parse storage_balance_of result");

                if balance != Value::Null {
                    log!("Account {} is registered, proceeding with transfer", signer);
                    self.perform_ft_transfer(signer, amount)
                } else {
                    log!("Account {} is not registered, registering now", signer);
                    let storage_deposit_amount = NearToken::from_millinear(1250);
                    assert!(
                        attached_deposit >= storage_deposit_amount,
                        "Insufficient deposit for storage registration, need at least 0.00125 NEAR"
                    );

                    Promise::new(self.token_contract.clone())
                        .function_call(
                            "storage_deposit".to_string(),
                            json!({ "account_id": signer, "registration_only": true })
                                .to_string()
                                .into_bytes(),
                            storage_deposit_amount,
                            Gas::from_tgas(30),
                        )
                        .then(
                            Self::ext(env::current_account_id())
                                .with_static_gas(Gas::from_tgas(60))
                                .on_storage_deposit_callback(signer, amount),
                        )
                }
            }
            PromiseResult::Failed => {
                log!("Failed to check storage balance for {}", signer);
                panic!("Storage balance check failed");
            }
        }
    }

    #[private]
    pub fn on_storage_deposit_callback(&mut self, signer: AccountId, amount: U128) -> Promise {
        assert_eq!(
            env::promise_results_count(),
            1,
            "Expected one promise result"
        );

        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                log!("Successfully registered {} with token contract", signer);
                self.perform_ft_transfer(signer, amount)
            }
            PromiseResult::Failed => {
                log!("Failed to register {} with token contract", signer);
                panic!("Storage deposit failed");
            }
        }
    }


    fn perform_ft_transfer(&self, receiver_id: AccountId, amount: U128) -> Promise {
        log!(
            "Initiating token transfer of {} for {}",
            amount.0,
            receiver_id
        );

        Promise::new(self.token_contract.clone())
            .function_call(
                "ft_transfer".to_string(),
                json!({
                    "receiver_id": receiver_id,
                    "amount": amount,
                })
                .to_string()
                .into_bytes(),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(50),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(10))
                    .on_token_transfer_callback(receiver_id, amount),
            )
    }

    #[private]
    pub fn on_nft_mint_callback(&mut self, donor_id: AccountId) {
        if env::promise_results_count() != 1 {
            log!("Unexpected number of promise results");
            return;
        }

        let donor = self.donors.get(&donor_id).expect("Donor not found");
        let channel_id = donor
            .reward_types
            .iter()
            .find_map(|r| match r {
                RewardType::NFT { channel_id, .. } => Some(channel_id.clone()),
                _ => None,
            })
            .expect("No NFT reward type");

        match env::promise_result(0) {
            PromiseResult::Successful(result) => {
                let token_id = String::from_utf8_lossy(&result).to_string();
                let new_reward_type = RewardType::NFT {
                    channel_id: channel_id.clone(),
                    token_id: token_id.clone(),
                };

                for i in 0..self.airdrop_records.len() {
                    let mut record = self.airdrop_records.get(i).unwrap();
                    if
                        record.recipient == donor_id &&
                        matches!(record.reward_type, RewardType::NFT { .. }) &&
                        !record.paid
                    {
                        record.reward_type = new_reward_type.clone();
                        record.paid = true;
                        self.airdrop_records.replace(i, &record);

                        let mut donor = donor.clone();
                        // Update reward_types to include the new token_id
                        if let Some(index) = donor.reward_types.iter().position(|r| matches!(r, RewardType::NFT { channel_id: c, .. } if c == &channel_id)) {
                            donor.reward_types[index] = new_reward_type.clone();
                        } else {
                            donor.reward_types.push(new_reward_type);
                        }
                        donor.paid = true;
                        self.donors.insert(&donor_id, &donor);

                        log!(
                            "Successfully updated airdrop record for donor {} with NFT token ID {} for donation_type {}",
                            donor_id,
                            token_id,
                            donor.donation_types.iter().last().map(|d| format!("{:?}", d)).unwrap_or_default()
                        );
                        return;
                    }
                }

                log!("No matching airdrop record found for donor {}", donor_id);
            }
            PromiseResult::Failed => {
                log!("NFT mint failed for donor {}", donor_id);
            }
        }
    }

    #[private]
    pub fn on_token_transfer_callback(&mut self, donor_id: AccountId, amount: U128) {
        if env::promise_results_count() != 1 {
            log!("Unexpected number of promise results");
            return;
        }

        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                let mut donor = self.donors.get(&donor_id).expect("Donor not found");
                for i in 0..self.airdrop_records.len() {
                    let mut record = self.airdrop_records.get(i).unwrap();
                    if
                        record.recipient == donor_id &&
                        record.amount == amount &&
                        matches!(record.reward_type, RewardType::Token) &&
                        !record.paid
                    {
                        record.paid = true;
                        self.airdrop_records.replace(i, &record);

                        donor.paid = true;
                        self.donors.insert(&donor_id, &donor);

                        log!(
                            "Successfully transferred {} tokens to donor {} for donation_type {}",
                            amount.0,
                            donor_id,
                            donor.donation_types.iter().last().map(|d| format!("{:?}", d)).unwrap_or_default()
                        );
                        return;
                    }
                }

                log!("No matching airdrop record found for donor {}", donor_id);
            }
            PromiseResult::Failed => {
                log!("Token transfer failed for donor {}", donor_id);
            }
        }
    }

    pub fn mark_payout_complete(&mut self, donor_id: AccountId) {
        self.assert_admin();
        let mut donor = self.donors.get(&donor_id).expect("Donor not found");
        assert!(!donor.paid, "Payout already completed");
        donor.paid = true;
        self.donors.insert(&donor_id, &donor);

        for i in 0..self.airdrop_records.len() {
            let mut record = self.airdrop_records.get(i).unwrap();
            if record.recipient == donor_id && !record.paid {
                record.paid = true;
                self.airdrop_records.replace(i, &record);
                break;
            }
        }
        log!("Marked payout complete for donor {}", donor_id);
    }

    #[payable]
    pub fn select_nft_reward(&mut self, channel_id: String, donation_type: DonationType) {
        let signer = env::predecessor_account_id();
        let mut donor = self.donors.get(&signer).expect("Donor not found");
        assert!(
            donor.reward_types.contains(&RewardType::Token),
            "Donor reward type does not include Token"
        );
        assert!(!donor.paid, "Payout already completed");
        match &donation_type {
            DonationType::Project { project_id } => assert!(!project_id.is_empty(), "Project ID must not be empty"),
            DonationType::Campaign { campaign_id } => assert!(campaign_id.len() <= 64, "Campaign ID must be 64 characters or less"),
            DonationType::Pot { pot_id } => assert!(env::is_valid_account_id(pot_id.as_bytes()), "Invalid pot_id"),
            DonationType::Direct => (),
        }

        let new_reward_type = RewardType::NFT {
            channel_id: channel_id.clone(),
            token_id: String::new(),
        };

        // Add donation_type if not already present
        if !donor.donation_types.contains(&donation_type) {
            donor.donation_types.push(donation_type.clone());
        }

        // Add reward_type if not already present
        if !donor.reward_types.contains(&new_reward_type) {
            donor.reward_types.push(new_reward_type);
        }

        self.donors.insert(&signer, &donor);
        log!("Donor {} selected NFT reward with channel_id {} for donation_type {:?}", signer, channel_id, donation_type);
    }

    pub fn get_donor(&self, wallet_id: AccountId) -> Option<Donor> {
        self.donors.get(&wallet_id)
    }

    pub fn get_donors(&self, start: u64, limit: u64) -> PaginatedDonors {
        assert!(limit > 0 && limit <= 100, "Limit must be between 1 and 100");
        let donors: Vec<Donor> = self.donors
            .values()
            .skip(start as usize)
            .take(limit as usize)
            .collect();
        let has_more = self.donors.len() > start + limit;
        PaginatedDonors { donors, has_more }
    }

    pub fn get_donors_by_donation_type(&self, donation_type: DonationType, start: u64, limit: u64) -> PaginatedDonors {
        assert!(limit > 0 && limit <= 100, "Limit must be between 1 and 100");
        let donors: Vec<Donor> = self.donors
            .values()
            .filter(|donor| donor.donation_types.contains(&donation_type))
            .skip(start as usize)
            .take(limit as usize)
            .collect();
        let total_matching = self
            .donors
            .values()
            .filter(|donor| donor.donation_types.contains(&donation_type))
            .count() as u64;
        let has_more = total_matching > start + limit;
        PaginatedDonors { donors, has_more }
    }

    pub fn get_airdrop_records(&self, start: u64, limit: u64) -> PaginatedAirdropRecords {
        assert!(limit > 0 && limit <= 100, "Limit must be between 1 and 100");
        let records: Vec<AirdropRecord> = self.airdrop_records
            .iter()
            .skip(start as usize)
            .take(limit as usize)
            .collect();
        let has_more = self.airdrop_records.len() > start + limit;
        PaginatedAirdropRecords { records, has_more }
    }

    pub fn get_airdrop_records_by_donation_type(
        &self,
        donation_type: DonationType,
        start: u64,
        limit: u64
    ) -> PaginatedAirdropRecords {
        assert!(limit > 0 && limit <= 100, "Limit must be between 1 and 100");
        let records: Vec<AirdropRecord> = self.airdrop_records
            .iter()
            .filter(|record| record.donation_type == donation_type)
            .skip(start as usize)
            .take(limit as usize)
            .collect();
        let total_matching = self.airdrop_records
            .iter()
            .filter(|record| record.donation_type == donation_type)
            .count() as u64;
        let has_more = total_matching > start + limit;
        PaginatedAirdropRecords { records, has_more }
    }

    pub fn get_project_rewards(&self, project_id: String) -> (U128, U128) {
        let total_donations = self.donors
            .values()
            .filter(|donor| donor.donation_types.iter().any(|d| matches!(d, DonationType::Project { project_id: ref id } if id == &project_id)))
            .map(|donor| donor.donation_amount.0)
            .sum::<u128>();
        let total_airdropped = self.airdrop_records
            .iter()
            .filter(|record| matches!(record.donation_type, DonationType::Project { project_id: ref id } if id == &project_id))
            .map(|record| record.amount.0)
            .sum::<u128>();
        (U128(total_donations), U128(total_airdropped))
    }

    pub fn get_total_distributed(&self) -> U128 {
        U128(self.total_distributed)
    }

    pub fn get_donor_count(&self) -> u64 {
        self.donors.len()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{VMContextBuilder, accounts};
    use near_sdk::testing_env;

    #[test]
    fn test_log_airdrop_multiple_donation_and_reward_types() {
        let context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None, None);

       
        contract.log_airdrop(
            accounts(1),
            "".to_string(),
            DonationType::Campaign { campaign_id: "campaign1".to_string() },
            U128(1),
        );

      
        let mut context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .build();
        context.attached_deposit = NearToken::from_yoctonear(2000);
        testing_env!(context);

       
        contract.log_airdrop(
            accounts(1),
            "channel123".to_string(),
            DonationType::Pot { pot_id: accounts(2) },
            U128(2),
        );

        let donor = contract.get_donor(accounts(1)).unwrap();

        assert_eq!(donor.wallet_id, accounts(1));
        assert_eq!(donor.airdrop_amount, U128(3));
        assert_eq!(donor.donation_amount, U128(3000));
        assert_eq!(donor.paid, false);

       
        assert_eq!(donor.donation_types.len(), 2);
        assert!(donor.donation_types.contains(&DonationType::Campaign { campaign_id: "campaign1".to_string() }));
        assert!(donor.donation_types.contains(&DonationType::Pot { pot_id: accounts(2) }));

       
        assert_eq!(donor.reward_types.len(), 2);
        assert!(donor.reward_types.contains(&RewardType::Token));
        assert!(donor.reward_types.iter().any(|r| matches!(r, RewardType::NFT { channel_id, token_id } if channel_id == "channel123" && token_id == "")));

        assert_eq!(contract.get_total_distributed(), U128(3));

        let records = contract.get_airdrop_records(0, 2).records;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].recipient, accounts(1));
        assert_eq!(records[0].amount, U128(1));
        assert_eq!(records[1].recipient, accounts(1));
        assert_eq!(records[1].amount, U128(2));
    }

    #[test]
    fn test_record_donation_multiple_types() {
        let context = VMContextBuilder::new()
            .predecessor_account_id(accounts(1))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None, None);

       
        contract.record_donation(DonationType::Direct);


        let mut context = VMContextBuilder::new()
            .predecessor_account_id(accounts(1))
            .build();
        context.attached_deposit = NearToken::from_yoctonear(2000);
        testing_env!(context);
        contract.record_donation(DonationType::Project { project_id: "project1".to_string() });

        let donor = contract.get_donor(accounts(1)).unwrap();

        assert_eq!(donor.donation_amount, U128(3000)); 
        assert_eq!(donor.donation_types.len(), 2);
        assert!(donor.donation_types.contains(&DonationType::Direct));
        assert!(donor.donation_types.contains(&DonationType::Project { project_id: "project1".to_string() }));
    }

    #[test]
    fn test_select_nft_reward_adds_types() {
        let context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None, None);

      
        contract.log_airdrop(
            accounts(1),
            "".to_string(),
            DonationType::Campaign { campaign_id: "campaign1".to_string() },
            U128(1),
        );

      
        let mut context = VMContextBuilder::new()
            .build();
        context.predecessor_account_id = accounts(1);
        testing_env!(context);
        contract.select_nft_reward("channel123".to_string(), DonationType::Direct);

        let donor = contract.get_donor(accounts(1)).unwrap();

        assert_eq!(donor.donation_types.len(), 2);
        assert!(donor.donation_types.contains(&DonationType::Campaign { campaign_id: "campaign1".to_string() }));
        assert!(donor.donation_types.contains(&DonationType::Direct));

        assert_eq!(donor.reward_types.len(), 2);
        assert!(donor.reward_types.contains(&RewardType::Token));
        assert!(donor.reward_types.iter().any(|r| matches!(r, RewardType::NFT { channel_id, token_id } if channel_id == "channel123" && token_id == "")));
    }

    #[test]
    fn test_get_donors_by_donation_type() {
        let context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None, None);

        contract.log_airdrop(accounts(1), "".to_string(), DonationType::Campaign { campaign_id: "campaign1".to_string() }, U128(1));
        let mut context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .build();
        context.attached_deposit = NearToken::from_yoctonear(2000);
        testing_env!(context);
        contract.log_airdrop(accounts(1), "channel123".to_string(), DonationType::Project { project_id: "project1".to_string() }, U128(1));
        contract.log_airdrop(accounts(2), "".to_string(), DonationType::Project { project_id: "project1".to_string() }, U128(1));

        let result = contract.get_donors_by_donation_type(DonationType::Campaign { campaign_id: "campaign1".to_string() }, 0, 1);
        assert_eq!(result.donors.len(), 1);
        assert_eq!(result.donors[0].wallet_id, accounts(1));
        assert_eq!(result.donors[0].donation_amount, U128(3000)); 
        assert!(!result.has_more);

        let result = contract.get_donors_by_donation_type(DonationType::Project { project_id: "project1".to_string() }, 0, 2);
        assert_eq!(result.donors.len(), 2);
        assert_eq!(result.donors[0].wallet_id, accounts(1));
        assert_eq!(result.donors[1].wallet_id, accounts(2));
        assert!(!result.has_more);
    }

    #[test]
    fn test_get_airdrop_records() {
        let context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None, None);

        contract.log_airdrop(accounts(1), "".to_string(), DonationType::Direct, U128(1));
        contract.log_airdrop(accounts(2), "channel123".to_string(), DonationType::Pot { pot_id: accounts(3) }, U128(1));

        let result = contract.get_airdrop_records(0, 1);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].recipient, accounts(1));
        assert!(result.has_more);

        let result = contract.get_airdrop_records(1, 1);
        assert_eq!(result.records.len(), 1);
        assert!(!result.has_more);
    }

    #[test]
    fn test_get_project_rewards() {
        let context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None, None);

        contract.log_airdrop(accounts(1), "".to_string(), DonationType::Project { project_id: "project1".to_string() }, U128(1));
        let mut context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .build();
        context.attached_deposit = NearToken::from_yoctonear(2000);
        testing_env!(context);
        contract.log_airdrop(accounts(2), "".to_string(), DonationType::Project { project_id: "project1".to_string() }, U128(1));

        let (total_donations, total_airdropped) = contract.get_project_rewards("project1".to_string());
        assert_eq!(total_donations, U128(3000));
        assert_eq!(total_airdropped, U128(2));
    }

    #[test]
    #[should_panic(expected = "Campaign ID must be 64 characters or less")]
    fn test_log_airdrop_invalid_campaign_id() {
        let context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None, None);

        let long_campaign_id = "a".repeat(65);
        contract.log_airdrop(accounts(1), "".to_string(), DonationType::Campaign { campaign_id: long_campaign_id }, U128(1));
    }

    #[test]
    #[should_panic(expected = "Donor not found")]
    fn test_select_nft_reward_no_donor() {
        let context = VMContextBuilder::new()
            .predecessor_account_id(accounts(1))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None, None);

        contract.select_nft_reward("channel123".to_string(), DonationType::Direct);
    }
}