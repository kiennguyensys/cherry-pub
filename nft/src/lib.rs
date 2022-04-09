/*!
Non-Fungible Token implementation with JSON serialization.
NOTES:
  - The maximum balance value is limited by U128 (2**128 - 1).
  - JSON calls should pass U128 as a base-10 string. E.g. "100".
  - The contract optimizes the inner trie structure by hashing account IDs. It will prevent some
    abuse of deep tries. Shouldn't be an issue, once NEAR clients implement full hashing of keys.
  - The contract tracks the change in storage before and after the call. If the storage increases,
    the contract requires the caller of the contract to attach enough deposit to the function call
    to cover the storage cost.
    This is done to prevent a denial of service attack on the contract by taking all available storage.
    If the storage decreases, the contract will issue a refund for the cost of the released storage.
    The unused tokens from the attached deposit are also refunded, so it's safe to
    attach more deposit than required.
  - To prevent the deployed contract from being modified or deleted, it should not have any access
    keys on its account.
*/
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata, NonFungibleTokenMetadataProvider, TokenMetadata, NFT_METADATA_SPEC,
};
use near_contract_standards::non_fungible_token::{Token, TokenId};
use near_contract_standards::non_fungible_token::NonFungibleToken;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::*;
use near_sdk::json_types::ValidAccountId;
use near_sdk::{
    env, near_bindgen, wee_alloc, AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseOrValue, Gas
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    tokens: NonFungibleToken,
    metadata: LazyOption<NFTContractMetadata>,
}

const DATA_IMAGE_SVG_NEAR_ICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 288 288'%3E%3Cg id='l' data-name='l'%3E%3Cpath d='M187.58,79.81l-30.1,44.69a3.2,3.2,0,0,0,4.75,4.2L191.86,103a1.2,1.2,0,0,1,2,.91v80.46a1.2,1.2,0,0,1-2.12.77L102.18,77.93A15.35,15.35,0,0,0,90.47,72.5H87.34A15.34,15.34,0,0,0,72,87.84V201.16A15.34,15.34,0,0,0,87.34,216.5h0a15.35,15.35,0,0,0,13.08-7.31l30.1-44.69a3.2,3.2,0,0,0-4.75-4.2L96.14,186a1.2,1.2,0,0,1-2-.91V104.61a1.2,1.2,0,0,1,2.12-.77l89.55,107.23a15.35,15.35,0,0,0,11.71,5.43h3.13A15.34,15.34,0,0,0,216,201.16V87.84A15.34,15.34,0,0,0,200.66,72.5h0A15.35,15.35,0,0,0,187.58,79.81Z'/%3E%3C/g%3E%3C/svg%3E";
const GAS_FOR_NFT_MINT: Gas = 25_000_000_000_000; 

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    NonFungibleToken,
    Metadata,
    TokenMetadata,
    Enumeration,
    Approval,
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract owned by `owner_id` with
    /// default metadata (for example purposes only).
    #[init]
    pub fn new_default_meta(owner_id: ValidAccountId) -> Self {
        Self::new(
            owner_id,
            NFTContractMetadata {
                spec: NFT_METADATA_SPEC.to_string(),
                name: "Cherry Pub Collection".to_string(),
                symbol: "CHEPU".to_string(),
                icon: Some(DATA_IMAGE_SVG_NEAR_ICON.to_string()),
                base_uri: None,
                reference: None,
                reference_hash: None,
            },
        )
    }

    #[init]
    pub fn new(owner_id: ValidAccountId, metadata: NFTContractMetadata) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        metadata.assert_valid();
        Self {
            tokens: NonFungibleToken::new(
                StorageKey::NonFungibleToken,
                owner_id,
                Some(StorageKey::TokenMetadata),
                Some(StorageKey::Enumeration),
                Some(StorageKey::Approval),
            ),
            metadata: LazyOption::new(StorageKey::Metadata, Some(&metadata)),
        }
    }

    /// Mint a new token with ID=`token_id` belonging to `receiver_id`.
    ///
    /// Since this example implements metadata, it also requires per-token metadata to be provided
    /// in this call. `self.tokens.mint` will also require it to be Some, since
    /// `StorageKey::TokenMetadata` was provided at initialization.
    ///
    /// `self.tokens.mint` will enforce `predecessor_account_id` to equal the `owner_id` given in
    /// initialization call to `new`.
    #[payable]
    pub fn nft_mint(
        &mut self,
        token_id: TokenId,
        receiver_id: ValidAccountId,
        token_metadata: TokenMetadata,
    ) -> Token {
        self.tokens.mint(token_id, receiver_id, Some(token_metadata))
    }

    pub fn owner_mint_collection(&mut self) -> u64 {
        assert_eq!(env::predecessor_account_id(), self.tokens.owner_id, "Only owner can mint collection");
        
        let mut i: u64 = 1;


        let lengend_edition_metadata = TokenMetadata {
            title: Some("Cherry Wine".into()),
            description: Some("a very-old wine glass made from sweet cherries".into()),
            media: Some("https://bafkreibdx7v3i2hgb7yy2qljzpuvp54pnj4bti6w44thqu6jsaxxwz6ncy.ipfs.nftstorage.link/".into()),
            media_hash: None,
            copies: Some(1u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None
        };
        while i <= 1 {
            self.nft_mint(i.to_string(), ValidAccountId::try_from(self.tokens.owner_id.clone()).unwrap(), lengend_edition_metadata.clone());
            i = i + 1;
        }

        let rare_edition_metadata = TokenMetadata {
            title: Some("Cherry Cocktail".into()),
            description: Some("a yummy cocktail made from fresh cherries".into()),
            media: Some("https://bafkreic76dlg7our5p7d3pe5ablvkacfqgr3otk7n4qwl7cx6sjdij6vgi.ipfs.nftstorage.link/".into()),
            media_hash: None,
            copies: Some(9u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None
        };
        while i > 1 && i <= 10 {
            self.nft_mint(i.to_string(), ValidAccountId::try_from(self.tokens.owner_id.clone()).unwrap(), rare_edition_metadata.clone());
            i = i + 1;
        }

        let common_edition_metadata = TokenMetadata {
            title: Some("Cherry Cake".into()),
            description: Some("A delicious cake".into()),
            media: Some("https://bafkreick2df2lyge67dxvb7nw5uajybtnvru46vg3hdewxoswd72oledji.ipfs.nftstorage.link/".into()),
            media_hash: None,
            copies: Some(40u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None
        };
        while i > 10 && i <= 50 {
            self.nft_mint(i.to_string(), ValidAccountId::try_from(self.tokens.owner_id.clone()).unwrap(), common_edition_metadata.clone());
            i = i + 1;
        }

        i
    }
}

near_contract_standards::impl_non_fungible_token_core!(Contract, tokens);
near_contract_standards::impl_non_fungible_token_approval!(Contract, tokens);
near_contract_standards::impl_non_fungible_token_enumeration!(Contract, tokens);

#[near_bindgen]
impl NonFungibleTokenMetadataProvider for Contract {
    fn nft_metadata(&self) -> NFTContractMetadata {
        self.metadata.get().unwrap()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, MockedBlockchain};

    use super::*;

    const MINT_STORAGE_COST: u128 = 5870000000000000000000;

    fn get_context(predecessor_account_id: ValidAccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    fn sample_token_metadata() -> TokenMetadata {
        TokenMetadata {
            title: Some("Cherry Wine".into()),
            description: Some("a very-old wine glass made from sweet cherries".into()),
            media: None,
            media_hash: None,
            copies: Some(1u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None,
        }
    }

    fn legend_edition_token_metadata() -> TokenMetadata {
        TokenMetadata {
            title: Some("Cherry Wine".into()),
            description: Some("a very-old wine glass made from sweet cherries".into()),
            media: Some("https://bafkreibdx7v3i2hgb7yy2qljzpuvp54pnj4bti6w44thqu6jsaxxwz6ncy.ipfs.nftstorage.link/".into()),
            media_hash: None,
            copies: Some(1u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None,
        }
    }

    fn rare_edition_token_metadata() -> TokenMetadata {
        TokenMetadata {
            title: Some("Cherry Cocktail".into()),
            description: Some("a yummy cocktail made from fresh cherries".into()),
            media: Some("https://bafkreic76dlg7our5p7d3pe5ablvkacfqgr3otk7n4qwl7cx6sjdij6vgi.ipfs.nftstorage.link/".into()),
            media_hash: None,
            copies: Some(9u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None,
        }
    }

    fn common_edition_token_metadata() -> TokenMetadata {
        TokenMetadata {
            title: Some("Cherry Cake".into()),
            description: Some("A delicious cake".into()),
            media: Some("https://bafkreick2df2lyge67dxvb7nw5uajybtnvru46vg3hdewxoswd72oledji.ipfs.nftstorage.link/".into()),
            media_hash: None,
            copies: Some(40u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None,
        }
    }

    #[test]
    fn test_new() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let contract = Contract::new_default_meta(accounts(1).into());
        testing_env!(context.is_view(true).build());
        assert_eq!(contract.nft_token("1".to_string()), None);
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let _contract = Contract::default();
    }

    #[test]
    fn test_mint_entire_collection() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1));

        testing_env!(
            context
            .account_balance(10u128.pow(24) * 100)
            .attached_deposit(MINT_STORAGE_COST * 50)
            .prepaid_gas(GAS_FOR_NFT_MINT * 10)
            .storage_usage(env::storage_usage())
            .predecessor_account_id(accounts(1))
            .build()
        );

        contract.owner_mint_collection();

        let supplies = contract.nft_total_supply();
        assert_eq!(supplies, U128::from(50));
    }

    #[test]
    fn test_mint() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1));

        testing_env!(
            context
            .attached_deposit(MINT_STORAGE_COST)
            .predecessor_account_id(accounts(1))
            .build()
        );

        let nft = contract.nft_mint("1".to_owned(), accounts(1), sample_token_metadata());
        assert_eq!(nft.owner_id, accounts(1).to_string());
        assert_eq!(nft.token_id, "1".to_owned());
        assert_eq!(nft.metadata.unwrap(), sample_token_metadata());
        assert_eq!(nft.approved_account_ids.unwrap(), HashMap::new());

    }

    #[test]
    #[should_panic(expected = "token_id must be unique")]
    fn test_mint_unique() {
        let mut context = get_context(accounts(0));
        testing_env!(context.build());

        let mut contract = Contract::new_default_meta(accounts(0));

        testing_env!(
            context
            .predecessor_account_id(accounts(0))
            .attached_deposit(MINT_STORAGE_COST * 2)
            .build()
        );

        let token_minted_1 = contract.nft_mint("0".to_owned(), accounts(0), sample_token_metadata());
        let token_minted_2 = contract.nft_mint("0".to_owned(), accounts(1), sample_token_metadata());
    }

    #[test]
    fn test_transfer() {
        let mut context = get_context(accounts(0));
        testing_env!(
            context
            .predecessor_account_id(accounts(0))
            .attached_deposit(MINT_STORAGE_COST)
            .build()
        );

        let mut contract = Contract::new_default_meta(accounts(0));
        let token_minted = contract.nft_mint("0".to_owned(), accounts(0), sample_token_metadata());

        testing_env!(
            context
            .predecessor_account_id(accounts(0))
            .attached_deposit(1)
            .build()
        );
        contract.nft_transfer(accounts(1), token_minted.token_id, None, None);

        let token_transfered = contract.nft_token("0".to_owned()).unwrap();
        assert_eq!(token_transfered.owner_id, accounts(1).to_string());
    }

    #[test]
    fn test_approve() {
        let mut context = get_context(accounts(0));

        testing_env!(
            context
            .predecessor_account_id(accounts(0))
            .attached_deposit(MINT_STORAGE_COST)
            .build()
        );

        let mut contract = Contract::new_default_meta(accounts(0));
        let token_id = contract.nft_mint("0".to_owned(), accounts(0), sample_token_metadata());

        contract.nft_approve(token_id.token_id, accounts(1), None);
        assert!(contract.nft_is_approved("0".to_owned(), accounts(1), None));
    }

    // #[test]
    // fn test_reject() {
    //     let mut context = get_context(accounts(0));
    //     testing_env!(
    //         context
    //         .predecessor_account_id(accounts(0))
    //         .attached_deposit(MINT_STORAGE_COST)
    //         .build()
    //     );

    //     let mut contract = Contract::new_default_meta(accounts(0));
    //     let token_minted = contract.nft_mint("0".to_owned(), accounts(0), sample_token_metadata());

    //     testing_env!(
    //         context
    //         .predecessor_account_id(accounts(0))
    //         .attached_deposit(10u128.pow(24))
    //         .build()
    //     );

    //     contract.nft_approve(token_minted.token_id, accounts(1), None);

    //     testing_env!(
    //         context
    //         .predecessor_account_id(accounts(0))
    //         .attached_deposit(1)
    //         .build()
    //     );
    //     contract.nft_transfer(accounts(2), "0".to_owned(), Some(1), None);
    //     contract.nft_revoke("0".to_owned(), accounts(1));
    // }

    #[test]
    fn test_revoke() {
        let mut context = get_context(accounts(0));
        testing_env!(
            context
            .attached_deposit(MINT_STORAGE_COST)
            .predecessor_account_id(accounts(0))
            .build()
        );

        let mut contract = Contract::new_default_meta(accounts(0));
        let token_minted = contract.nft_mint("0".to_owned(), accounts(0), sample_token_metadata());

        testing_env!(
            context
            .predecessor_account_id(accounts(0))
            .attached_deposit(10u128.pow(24))
            .build()
        );

        contract.nft_approve(token_minted.token_id, accounts(1), None);

        testing_env!(
            context
            .predecessor_account_id(accounts(0))
            .attached_deposit(1)
            .build()
        );
        contract.nft_revoke("0".to_owned(), accounts(1));

        assert!(!contract.nft_is_approved("0".to_owned(), accounts(1), None));

    }

    #[test]
    fn test_revoke_all() {
        let mut context = get_context(accounts(0));
        testing_env!(
            context
            .attached_deposit(MINT_STORAGE_COST)
            .predecessor_account_id(accounts(0))
            .build()
        );

        let mut contract = Contract::new_default_meta(accounts(0));
        let token_minted = contract.nft_mint("0".to_owned(), accounts(0), sample_token_metadata());

        testing_env!(
            context
            .predecessor_account_id(accounts(0))
            .attached_deposit(10u128.pow(24))
            .build()
        );

        contract.nft_approve(token_minted.clone().token_id, accounts(1), None);
        contract.nft_approve(token_minted.clone().token_id, accounts(2), None);

        testing_env!(
            context
            .predecessor_account_id(accounts(0))
            .attached_deposit(1)
            .build()
        );
        contract.nft_revoke_all("0".to_owned());

        let token = contract.nft_token("0".to_owned()).unwrap();
        
        assert_eq!(token.approved_account_ids.unwrap(), HashMap::new());
    }
}

