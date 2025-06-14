use near_sdk::borsh::{ self, BorshDeserialize, BorshSerialize };
use near_sdk::collections::{ UnorderedMap, Vector };
use near_sdk::json_types::U128;
use near_sdk::{ env, log, near_bindgen, AccountId, Promise, Gas };
use near_sdk::serde::{ Deserialize, Serialize };
use serde_json::{ json };
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
    #[schemars(with = "String")]
    pub campaign_id: String,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, JsonSchema, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum RewardType {
    Token,
    NFT {
        channel_id: String,
        token_id: String,
    },
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
    pub reward_type: RewardType,
    #[schemars(with = "String")]
    pub campaign_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(crate = "near_sdk::serde")]
pub struct PaginatedAirdropRecords {
    pub records: Vec<AirdropRecord>,
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
        }
    }
}

#[near_bindgen]
impl DonorPayouts {
    #[init]
    pub fn new(potlock_nfts_contract: Option<AccountId>) -> Self {
        let admin = env::predecessor_account_id();
        Self {
            donors: UnorderedMap::new(b"d"),
            airdrop_records: Vector::new(b"a"),
            total_distributed: 0,
            admin,
            potlock_nfts_contract: potlock_nfts_contract.unwrap_or(
                "potlock-nfts.testnet".parse().unwrap()
            ),
        }
    }

    #[payable]
    pub fn log_airdrop(&mut self, recipient: AccountId, channel_id: String, campaign_id: String) {
        //self.assert_admin();
        let amount_u128: u128 = U128(1).into();
        let attached_amount = env::attached_deposit().as_yoctonear();
        assert!(campaign_id.len() <= 64, "Campaign ID must be 64 characters or less");

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
            amount: U128(1),
            timestamp: env::block_timestamp(),
            paid: false,
            reward_type: reward_type.clone(),
            campaign_id: campaign_id.clone(),
        };
        self.airdrop_records.push(&record);

        let mut donor = self.donors.get(&recipient).unwrap_or(Donor {
            wallet_id: recipient.clone(),
            donation_amount: U128(0),
            airdrop_amount: U128(0),
            paid: false,
            reward_type: reward_type.clone(),
            campaign_id: campaign_id.clone(),
        });

        donor.airdrop_amount = U128(donor.airdrop_amount.0 + amount_u128);
        donor.reward_type = reward_type;
        donor.campaign_id = campaign_id.clone();
        donor.donation_amount = U128(donor.donation_amount.0 + attached_amount);

        self.donors.insert(&recipient, &donor);
        self.total_distributed += amount_u128;

        log!("Logged airdrop for {}: {} tokens, campaign {}", recipient, amount_u128, campaign_id);
    }

    #[payable]
    pub fn send_nft_reward(&mut self) -> Promise {
        let signer = env::predecessor_account_id();
        let donor = self.donors.get(&signer).expect("Donor not found");
        assert!(!donor.paid, "Payout already completed");

        let channel_id = match &donor.reward_type {
            RewardType::NFT { channel_id, .. } => channel_id.clone(),
            _ => panic!("Donor reward type is not NFT"),
        };

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

    #[private]
    pub fn on_nft_mint_callback(&mut self, donor_id: AccountId) {
        if env::promise_results_count() != 1 {
            log!("Unexpected number of promise results");
            return;
        }

        let donor = self.donors.get(&donor_id).expect("Donor not found");
        let channel_id = match &donor.reward_type {
            RewardType::NFT { channel_id, .. } => channel_id.clone(),
            _ => {
                log!("Invalid reward type for donor {}", donor_id);
                return;
            }
        };

        match env::promise_result(0) {
            PromiseResult::Successful(result) => {
                let token_id = String::from_utf8_lossy(&result).to_string();

                for i in 0..self.airdrop_records.len() {
                    let mut record = self.airdrop_records.get(i).unwrap();
                    if
                        record.recipient == donor_id &&
                        record.campaign_id == donor.campaign_id &&
                        matches!(record.reward_type, RewardType::NFT { .. }) &&
                        !record.paid
                    {
                        record.reward_type = RewardType::NFT {
                            channel_id: channel_id.clone(),
                            token_id: token_id.clone(),
                        };
                        record.paid = true;
                        self.airdrop_records.replace(i, &record);

                        let mut donor = donor.clone();
                        donor.reward_type = RewardType::NFT {
                            channel_id: channel_id.clone(),
                            token_id: token_id.clone(),
                        };
                        donor.paid = true;
                        self.donors.insert(&donor_id, &donor);

                        log!(
                            "Successfully updated airdrop record for donor {} with NFT token ID {} for campaign {}",
                            donor_id,
                            token_id,
                            donor.campaign_id
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

    pub fn get_donor(&self, wallet_id: AccountId) -> Option<Donor> {
        self.donors.get(&wallet_id)
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

    pub fn get_airdrop_records_by_campaign(
        &self,
        campaign_id: String,
        start: u64,
        limit: u64
    ) -> PaginatedAirdropRecords {
        assert!(limit > 0 && limit <= 100, "Limit must be between 1 and 100");
        let records: Vec<AirdropRecord> = self.airdrop_records
            .iter()
            .filter(|record| record.campaign_id == campaign_id)
            .skip(start as usize)
            .take(limit as usize)
            .collect();
        let total_matching = self.airdrop_records
            .iter()
            .filter(|record| record.campaign_id == campaign_id)
            .count() as u64;
        let has_more = total_matching > start + limit;
        PaginatedAirdropRecords { records, has_more }
    }

    pub fn get_total_distributed(&self) -> U128 {
        U128(self.total_distributed)
    }

    pub fn get_donor_count(&self) -> u64 {
        self.donors.len()
    }

    // fn assert_admin(&self) {
    //     assert_eq!(env::predecessor_account_id(), self.admin, "Only admin can call this function");
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{ VMContextBuilder, accounts };
    use near_sdk::testing_env;
    use near_sdk::NearToken;

    #[test]
    fn test_log_airdrop_and_get_donor_token() {
        let  context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None);

        contract.log_airdrop(accounts(1), "".to_string(), "campaign1".to_string());
        let donor = contract.get_donor(accounts(1)).unwrap();

        assert_eq!(donor.wallet_id, accounts(1));
        assert_eq!(donor.airdrop_amount, U128(1));
        assert_eq!(donor.donation_amount, U128(1000));
        assert_eq!(donor.paid, false);
        assert_eq!(donor.campaign_id, "campaign1");
        assert!(matches!(donor.reward_type, RewardType::Token));
        assert_eq!(contract.get_total_distributed(), U128(1));

        let records = contract.get_airdrop_records(0, 1).records;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].recipient, accounts(1));
        assert_eq!(records[0].amount, U128(1));
        assert_eq!(records[0].campaign_id, "campaign1");
        assert!(matches!(records[0].reward_type, RewardType::Token));
    }

    #[test]
    fn test_log_airdrop_nft() {
        let  context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(2000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None);

        contract.log_airdrop(accounts(1), "channel123".to_string(), "campaign1".to_string());
        let donor = contract.get_donor(accounts(1)).unwrap();

        assert_eq!(donor.wallet_id, accounts(1));
        assert_eq!(donor.airdrop_amount, U128(1));
        assert_eq!(donor.donation_amount, U128(2000));
        assert_eq!(donor.paid, false);
        assert_eq!(donor.campaign_id, "campaign1");
        if let RewardType::NFT { channel_id, token_id } = &donor.reward_type {
            assert_eq!(channel_id, "channel123");
            assert_eq!(token_id, "");
        } else {
            panic!("Expected NFT reward type");
        }

        assert_eq!(contract.get_total_distributed(), U128(1));

        let records = contract.get_airdrop_records(0, 1).records;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].recipient, accounts(1));
        assert_eq!(records[0].amount, U128(1));
        assert_eq!(records[0].campaign_id, "campaign1");
        if let RewardType::NFT { channel_id, token_id } = &records[0].reward_type {
            assert_eq!(channel_id, "channel123");
            assert_eq!(token_id, "");
        } else {
            panic!("Expected NFT reward type in record");
        }
    }

   


    #[test]
    fn test_get_airdrop_records() {
        let  context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None);

        contract.log_airdrop(accounts(1), "".to_string(), "campaign1".to_string());
        contract.log_airdrop(accounts(2), "channel123".to_string(), "campaign1".to_string());

        let result = contract.get_airdrop_records(0, 1);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].recipient, accounts(1));
        assert!(result.has_more);

        let result = contract.get_airdrop_records(1, 1);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].recipient, accounts(2));
        assert!(!result.has_more);
    }

    #[test]
    fn test_get_airdrop_records_by_campaign() {
        let  context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None);

        contract.log_airdrop(accounts(1), "".to_string(), "campaign1".to_string());
        contract.log_airdrop(accounts(2), "channel123".to_string(), "campaign2".to_string());

        let result = contract.get_airdrop_records_by_campaign("campaign1".to_string(), 0, 1);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].recipient, accounts(1));
        assert!(!result.has_more);

        let result = contract.get_airdrop_records_by_campaign("campaign2".to_string(), 0, 1);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].recipient, accounts(2));
        assert!(!result.has_more);
    }

    #[test]
    #[should_panic(expected = "Campaign ID must be 64 characters or less")]
    fn test_log_airdrop_invalid_campaign_id() {
        let  context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None);

        let long_campaign_id = "a".repeat(65);
        contract.log_airdrop(accounts(1), "".to_string(), long_campaign_id);
    }

    #[test]
    #[should_panic(expected = "Donor not found")]
    fn test_send_nft_reward_no_donor() {
        let  context = VMContextBuilder::new()
            .predecessor_account_id(accounts(1))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None);

        contract.send_nft_reward();
    }

    #[test]
    #[should_panic(expected = "Donor reward type is not NFT")]
    fn test_send_nft_reward_wrong_reward_type() {
        let  context = VMContextBuilder::new()
            .predecessor_account_id(accounts(0))
            .attached_deposit(NearToken::from_yoctonear(1000))
            .build();
        testing_env!(context);
        let mut contract = DonorPayouts::new(None);

        contract.log_airdrop(accounts(1), "".to_string(), "campaign1".to_string());
        testing_env!(
            VMContextBuilder::new()
                .predecessor_account_id(accounts(1))
                .attached_deposit(NearToken::from_yoctonear(1000))
                .build()
        );
        contract.send_nft_reward();
    }
}
