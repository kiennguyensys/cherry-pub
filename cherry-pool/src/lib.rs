
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, Vector},
    env, ext_contract,
    json_types::{ValidAccountId, U128, U64, Base58PublicKey},
    log, near_bindgen,
    serde::{Deserialize, Serialize},
    wee_alloc, AccountId, Balance, EpochHeight, PanicOnDefault, 
    Promise, PromiseOrValue,PromiseResult,
    PublicKey
};
use near_contract_standards::non_fungible_token::{Token, TokenId};
// use std::convert::TryFrom;
use uint::construct_uint;
mod internal;
mod test_utils;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

const STAKE_SHARE_PRICE_GUARANTEE_FUND: Balance = 1_000_000_000_000;
const MIN_TICKET_DEPOSIT_PRICE: Balance = 10 * 10u128.pow(24);
const POOL_THRESHOLD: Balance = 10_000 * 10u128.pow(24);

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct StakingPool {
    owner_id: AccountId,
    accounts: LookupMap<String, StakingPoolAccount>,
    tickets_count: u64,
    tickets_random_slots: Vector<AccountId>,
    next_prize_event_epoch_height: EpochHeight,
    total_reward: Balance,
    total_staked_balance: Balance,
    last_total_balance: Balance,
    total_stake_shares: Balance,
    stake_public_key: PublicKey,
    last_epoch_height: EpochHeight,
    is_restake_paused: bool,
    nft_contract_id: AccountId,
    nft_hold: Vector<TokenId>
}

/// staking pool interface that STAKE token contract depends on
#[near_bindgen]
impl StakingPool {
    #[init]
    pub fn new(owner_id: AccountId, stake_public_key: Base58PublicKey) -> Self {
        let account_balance = env::account_balance();
        let total_staked_balance = account_balance - STAKE_SHARE_PRICE_GUARANTEE_FUND;
        assert_eq!(
            env::account_locked_balance(),
            0,
            "The staking pool shouldn't be staking at the initialization"
        );
        let mut this = Self {
            owner_id: owner_id,
            accounts: LookupMap::new(b"a".to_vec()),
            tickets_random_slots: Vector::new(b"t".to_vec()),
            tickets_count: 0,
            next_prize_event_epoch_height: env::epoch_height() + 14,
            total_reward: 0,
            total_staked_balance: total_staked_balance,
            last_total_balance: account_balance,
            total_stake_shares: total_staked_balance,
            stake_public_key: stake_public_key.into(),
            last_epoch_height: env::epoch_height(),
            is_restake_paused: false,
            nft_contract_id: "cherrypub_nft.testnet".to_owned(),
            nft_hold: Vector::new(b"h".to_vec())
        };
        this
    }

    pub fn get_account(&self, account_id: AccountId) -> StakingPoolAccount {
        let mut account = 
            self.accounts
            .get(&account_id)
            .unwrap_or_else(|| StakingPoolAccount::new(&account_id));
        account.staked_balance = 
            self
            .staked_amount_from_num_shares_rounded_down(account.stake_shares)
            .into();
        account.can_withdraw =
            account.unstaked_available_epoch_height <= env::epoch_height();
        account
    }

    /// Distributes rewards and restakes if needed.
    pub fn ping(&mut self) {
        if self.internal_ping() {
            self.internal_restake();
        }
    }

    #[payable]
    pub fn deposit(&mut self) {
        let need_to_restake = self.internal_ping();
        let amount = env::attached_deposit();
        let mut account = self.get_account(env::predecessor_account_id());
        account.unstaked_balance += amount;
        self.last_total_balance += amount;
        self.save_account(&account);

        if need_to_restake {
            self.internal_restake();
        }
    }

    pub fn stake(&mut self, amount: Balance) {
        self.internal_ping();
        self.internal_stake(amount);
        self.internal_add_tickets(amount, 1);
        self.internal_restake();
    }

    #[payable]
    pub fn deposit_and_stake(&mut self) {
        self.deposit();
        self.stake(env::attached_deposit());
    }

    pub fn withdraw_all(&mut self) {
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.can_withdraw, "account cannot withdraw yet");
        assert!(account.unstaked_balance > 0, "unstaked balance is zero");
        let unstaked_balance = account.unstaked_balance;
        account.unstaked_balance = 0;
        self.save_account(&account);
        Promise::new(account.account_id).transfer(unstaked_balance);
    }

    pub fn unstake(&mut self, amount: Balance) {
        self.internal_ping();
        self.internal_unstake(amount);
        self.internal_remove_tickets(amount);
        self.internal_restake();
    }

    pub fn unstake_all(&mut self) {
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.staked_balance > 0, "Staked balance is zero");
        account.unstaked_balance += account.staked_balance;
        self.total_staked_balance -= account.staked_balance;
        account.staked_balance = 0;
        self.save_account(&account);
    }

    // *** View Method
    /// Returns the unstaked balance of the given account.
    pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> u128 {
        self.get_account(account_id).unstaked_balance
    }

    /// Returns the staked balance of the given account.
    /// NOTE: This is computed from the amount of "stake" shares the given account has and the
    /// current amount of total staked balance and total stake shares on the account.
    pub fn get_account_staked_balance(&self, account_id: AccountId) -> u128 {
        self.get_account(account_id).staked_balance
    }

    /// current numbers of tickets on the account
    pub fn get_account_tickets_amount(&self, account_id: AccountId) -> u64 {
        self.get_account(account_id).tickets_amount
    }

    /// Returns the total balance of the given account (including staked and unstaked balances).
    pub fn get_account_total_balance(&self, account_id: AccountId) -> u128 {
        let account = self.get_account(account_id);
        (account.unstaked_balance + account.staked_balance).into()
    }

    /// Returns `true` if the given account can withdraw tokens in the current epoch.
    pub fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool {
        self.get_account(account_id).can_withdraw
    }

    pub fn on_stake_action(&mut self) {
        assert_eq!(
            env::current_account_id(),
            env::predecessor_account_id(),
            "Can be called only as a callback"
        );

        assert_eq!(
            env::promise_results_count(),
            1,
            "Contract expected a result on the callback"
        );
        let stake_action_succeeded = match env::promise_result(0) {
            PromiseResult::Successful(_) => true,
            _ => false,
        };

        // If the stake action failed and the current locked amount is positive, then the contract
        // has to unstake.
        if !stake_action_succeeded && env::account_locked_balance() > 0 {
            Promise::new(env::current_account_id()).stake(0, self.stake_public_key.clone());
        }
    }
}

/// exposed to support simulation testing
#[near_bindgen]
impl StakingPool {
    pub fn update_account(&mut self, account: StakingPoolAccount) {
    }
}

impl StakingPool {
    fn save_account(&mut self, account: &StakingPoolAccount) {
        self.accounts.insert(&account.account_id, account);
    }
}

/// Interface for the contract itself.
#[ext_contract(ext_self)]
pub trait SelfContract {
    /// A callback to check the result of the staking action.
    /// In case the stake amount is less than the minimum staking threshold, the staking action
    /// fails, and the stake amount is not changed. This might lead to inconsistent state and the
    /// follow withdraw calls might fail. To mitigate this, the contract will issue a new unstaking
    /// action in case of the failure of the first staking action.
    fn on_stake_action(&mut self);
    fn nft_valid_callback(&self) -> bool;
    fn nft_transfer_callback(&mut self, token_id: TokenId) -> bool;
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolAccount {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: Balance,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: Balance,
    /// The amount of shares
    pub stake_shares: Balance,
    /// Updated when stake/unstake actions happen, used for reward
    pub stake_points: u64,
    /// The amount of tickets
    pub tickets_amount: u64,
    /// Bonus multiplier by using special NFT
    pub tickets_multiplier: u64,
    /// Whether the unstaked balance is available for withdrawal now.
    pub can_withdraw: bool,
    pub unstaked_available_epoch_height: EpochHeight,
}

impl StakingPoolAccount {
    pub fn new(account_id: &str) -> Self {
        StakingPoolAccount {
            account_id: account_id.to_string(),
            unstaked_balance: 0,
            staked_balance: 0,
            stake_shares: 0,
            stake_points: 0,
            tickets_multiplier: 1,
            tickets_amount: 0,
            can_withdraw: false,
            unstaked_available_epoch_height: 0
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use near_sdk::{serde_json, testing_env, MockedBlockchain, VMContext};

    use crate::test_utils::*;

    use super::*;

    struct Emulator {
        pub contract: StakingPool,
        pub epoch_height: EpochHeight,
        pub amount: Balance,
        pub locked_amount: Balance,
        last_total_staked_balance: Balance,
        last_total_stake_shares: Balance,
        context: VMContext,
    }

    impl Emulator {
        pub fn new(
            owner: String,
            stake_public_key: String,
        ) -> Self {
            let context = VMContextBuilder::new()
                .current_account_id(owner.clone())
                .account_balance(ntoy(30))
                .finish();
            testing_env!(context.clone());
            let contract = StakingPool::new(
                owner,
                Base58PublicKey::try_from(stake_public_key).unwrap(),
            );
            let last_total_staked_balance = contract.total_staked_balance;
            let last_total_stake_shares = contract.total_stake_shares;
            Emulator {
                contract,
                epoch_height: 0,
                amount: ntoy(30),
                locked_amount: 0,
                last_total_staked_balance,
                last_total_stake_shares,
                context,
            }
        }

        fn verify_stake_price_increase_guarantee(&mut self) {
            let total_staked_balance = self.contract.total_staked_balance;
            let total_stake_shares = self.contract.total_stake_shares;

            assert!(
                U256::from(total_staked_balance) * U256::from(self.last_total_stake_shares)
                    >= U256::from(self.last_total_staked_balance) * U256::from(total_stake_shares),
                "Price increase guarantee was violated."
            );
            self.last_total_staked_balance = total_staked_balance;
            self.last_total_stake_shares = total_stake_shares;

        }

        pub fn update_context(&mut self, predecessor_account_id: String, deposit: Balance) {
            self.verify_stake_price_increase_guarantee();
            self.context = VMContextBuilder::new()
                .current_account_id(staking())
                .predecessor_account_id(predecessor_account_id.clone())
                .signer_account_id(predecessor_account_id)
                .attached_deposit(deposit)
                .account_balance(self.amount)
                .account_locked_balance(self.locked_amount)
                .epoch_height(self.epoch_height)
                .finish();
            testing_env!(self.context.clone());
            println!(
                "Epoch: {}, Deposit: {}, amount: {}, locked_amount: {}",
                self.epoch_height, deposit, self.amount, self.locked_amount
            );
        }

        pub fn simulate_stake_call(&mut self) {
            let total_stake = self.contract.total_staked_balance;
            // Stake action
            self.amount = self.amount + self.locked_amount - total_stake;
            self.locked_amount = total_stake;
            // Second function call action
            self.update_context(staking(), 0);
        }

        pub fn skip_epochs(&mut self, num: EpochHeight) {
            self.epoch_height += num;
            self.locked_amount = (self.locked_amount * (100 + u128::from(num))) / 100;
        }
    }

    #[test]
    fn test_restake_fail() {
        let mut emulator = Emulator::new(
            owner(),
            "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7".to_string(),
            // zero_fee(),
        );
        emulator.update_context(bob(), 0);
        emulator.contract.internal_restake();
        // let receipts = env::created_receipts();
        // assert_eq!(receipts.len(), 2);
        // // Mocked Receipt fields are private, so can't check directly.
        // assert!(serde_json::to_string(&receipts[0])
        //     .unwrap()
        //     .contains("\"actions\":[{\"Stake\":{\"stake\":29999999999999000000000000,"));
        // assert!(serde_json::to_string(&receipts[1])
        //     .unwrap()
        //     .contains("\"method_name\":\"on_stake_action\""));
        emulator.simulate_stake_call();

        emulator.update_context(staking(), 0);
        testing_env_with_promise_results(emulator.context.clone(), PromiseResult::Failed);
        emulator.contract.on_stake_action();
        // let receipts = env::created_receipts();
        // assert_eq!(receipts.len(), 1);
        // assert!(serde_json::to_string(&receipts[0])
        //     .unwrap()
        //     .contains("\"actions\":[{\"Stake\":{\"stake\":0,"));
    }

    #[test]
    fn test_stake_unstake() {
        let mut emulator = Emulator::new(
            owner(),
            "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7".to_string(),
            // zero_fee(),
        );
        let deposit_amount = ntoy(10_000);
        emulator.update_context(bob(), deposit_amount);
        emulator.contract.deposit();
        emulator.amount += deposit_amount;
        emulator.update_context(bob(), 0);
        emulator.contract.stake(deposit_amount.into());
        emulator.simulate_stake_call();
        assert_eq!(
            emulator.contract.get_account_staked_balance(bob()),
            deposit_amount
        );
        let locked_amount = emulator.locked_amount;
        // 10 epochs later, unstake half of the money.
        emulator.skip_epochs(10);
        // Overriding rewards
        emulator.locked_amount = locked_amount + ntoy(10);
        emulator.update_context(bob(), 0);
        emulator.contract.ping();
        println!("Total Stake shares: {}", emulator.contract.total_stake_shares);
        println!("Total Stake Balance: {}", emulator.contract.total_staked_balance);
        assert_eq_in_near!(
            emulator.contract.get_account_staked_balance(bob()),
            deposit_amount + ntoy(10)
        );
        emulator.contract.unstake((deposit_amount / 2).into());
        emulator.simulate_stake_call();
        assert_eq_in_near!(
            emulator.contract.get_account_staked_balance(bob()),
            deposit_amount / 2 + ntoy(10)
        );
        assert_eq_in_near!(
            emulator.contract.get_account_unstaked_balance(bob()),
            deposit_amount / 2
        );
        let acc = emulator.contract.get_account(bob());
        assert_eq!(acc.account_id, bob());
        assert_eq_in_near!(acc.unstaked_balance, deposit_amount / 2);
        assert_eq_in_near!(acc.staked_balance, deposit_amount / 2 + ntoy(10));
        assert!(!acc.can_withdraw);

        assert!(!emulator
            .contract
            .is_account_unstaked_balance_available(bob()),);
        emulator.skip_epochs(4);
        emulator.update_context(bob(), 0);
        assert!(emulator
            .contract
            .is_account_unstaked_balance_available(bob()),);
    }

    #[test]
    fn test_add_remove_tickets() {
        let mut emulator = Emulator::new(owner(), "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7".to_owned());

        let deposit_amount = ntoy(1_000);
        emulator.update_context(bob(), deposit_amount);
        emulator.contract.deposit();
        emulator.amount += deposit_amount;
        emulator.update_context(bob(), 0);

        emulator.contract.stake(deposit_amount.into());
        emulator.simulate_stake_call();
        assert_eq!(
            emulator.contract.get_account_tickets_amount(bob()),
            1_00
        );

        emulator.skip_epochs(4);
        emulator.update_context(bob(), 0);

        emulator.contract.unstake((deposit_amount / 2).into());
        emulator.simulate_stake_call();
        assert_eq!(
            emulator.contract.get_account_tickets_amount(bob()),
            50
        );
    }

    #[test]
    fn test_winner_announcement() {
        let mut emulator = Emulator::new(owner(), "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7".to_owned());

        let deposit_amount = ntoy(1_00);
        emulator.update_context(bob(), deposit_amount);
        emulator.contract.deposit();
        emulator.amount += deposit_amount;
        emulator.update_context(bob(), 0);

        emulator.contract.stake(deposit_amount.into());
        emulator.simulate_stake_call();
        assert_eq!(
            emulator.contract.get_account_tickets_amount(bob()),
            10
        );

        emulator.skip_epochs(14);
        emulator.update_context(bob(), 0);

        let winner = emulator.contract.get_prize_winner();
        assert_eq!(winner, bob());

        let contract_balance = emulator.amount;
        let reward = emulator.contract.total_reward;
        let promise = emulator.contract.transfer_prize_to_winner(winner);

        emulator.update_context(bob(), 0);

        println!("{}", emulator.contract.total_staked_balance);
        assert_eq!(emulator.amount, contract_balance - reward);
    }

    // #[test]
    fn test_rewards() {
        let mut emulator = Emulator::new(
            owner(),
            "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7".to_string()
        );
        let initial_balance = ntoy(100);
        emulator.update_context(alice(), initial_balance);
        emulator.contract.deposit();
        emulator.amount += initial_balance;
        let mut remaining = 100;
        let mut amount = 1;
        while remaining >= 4 {
            emulator.skip_epochs(3);
            emulator.update_context(alice(), 0);
            emulator.contract.ping();
            emulator.update_context(alice(), 0);
            amount = 2 + (amount - 1) % 3;
            emulator.contract.stake(ntoy(amount).into());
            emulator.simulate_stake_call();
            remaining -= amount;
        }
    }
}
