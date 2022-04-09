use crate::*;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::convert::TryInto;
const ON_STAKE_ACTION_GAS: u64 = 20_000_000_000_000;
const NO_DEPOSIT: Balance = 0;
const NUM_EPOCHS_TO_UNLOCK: EpochHeight = 4;



impl StakingPool {
    pub(crate) fn internal_ping(&mut self ) -> bool {
        let epoch_height = env::epoch_height();
        if self.last_epoch_height == epoch_height {
            return false;
        }
        self.last_epoch_height = epoch_height;

        // New total amount (both locked and unlocked balances).
        // NOTE: We need to subtract `attached_deposit` in case `ping` called from `deposit` call
        // since the attached deposit gets included in the `account_balance`, and we have not
        // accounted it yet.
        let total_balance =
            env::account_locked_balance() + env::account_balance() - env::attached_deposit();

        assert!(
            total_balance >= self.last_total_balance,
            "The new total balance should not be less than the old total balance"
        );
        let total_reward = total_balance - self.last_total_balance;

        if total_reward > 0 {
            // The validation fee that the contract owner takes.
            // let owners_fee = self.reward_fee_fraction.multiply(total_reward);

            // Distributing the remaining reward to the delegators first.
            // let remaining_reward = total_reward - owners_fee;
            self.total_reward += total_reward;
            self.total_staked_balance += total_reward;

            // Now buying "stake" shares for the contract owner at the new share price.
            // let num_shares = self.num_shares_from_staked_amount_rounded_down(owners_fee);
            // if num_shares > 0 {
            //     // Updating owner's inner account
            //     let owner_id = self.owner_id.clone();
            //     let mut account = self.internal_get_account(&owner_id);
            //     account.stake_shares += num_shares;
            //     self.internal_save_account(&owner_id, &account);
            //     // Increasing the total amount of "stake" shares.
            //     self.total_stake_shares += num_shares;
            // }
            // Increasing the total staked balance by the owners fee, no matter whether the owner
            // received any shares or not.
            // self.total_staked_balance += owners_fee;

            env::log(
                format!(
                    "Epoch {}: Contract received total rewards of {} tokens. New total staked balance is {}",
                    epoch_height, total_reward, self.total_staked_balance,
                )
                    .as_bytes(),
            );
            // if num_shares > 0 {
            //     env::log(format!("Total rewards fee is {} stake shares.", num_shares).as_bytes());
            // }
        }

        if self.next_prize_event_epoch_height <= env::epoch_height() {
            let winner = self.get_prize_winner();
            self.transfer_prize_to_winner(winner);
            self.next_prize_event_epoch_height = env::epoch_height() + 14;
        }

        self.last_total_balance = total_balance;
        true
    }

    pub(crate) fn internal_stake(&mut self, amount: Balance) {
        assert!(amount > 0, "Staking amount should be positive");

        let account_id = env::predecessor_account_id();
        let mut account = self.get_account(account_id);

        // Calculate the number of "stake" shares that the account will receive for staking the
        // given amount.
        let num_shares = self.num_shares_from_staked_amount_rounded_down(amount);
        assert!(
            num_shares > 0,
            "The calculated number of \"stake\" shares received for staking should be positive"
        );
        // The amount of tokens the account will be charged from the unstaked balance.
        // Rounded down to avoid overcharging the account to guarantee that the account can always
        // unstake at least the same amount as staked.
        let charge_amount = self.staked_amount_from_num_shares_rounded_down(num_shares);
        assert!(
            charge_amount > 0,
            "Invariant violation. Calculated staked amount must be positive, because \"stake\" share price should be at least 1"
        );

        assert!(
            account.unstaked_balance >= charge_amount,
            "Not enough unstaked balance to stake"
        );
        account.unstaked_balance -= charge_amount;
        account.stake_shares += num_shares;
        account.stake_points += 1;
        self.save_account(&account);

        // The staked amount that will be added to the total to guarantee the "stake" share price
        // never decreases. The difference between `stake_amount` and `charge_amount` is paid
        // from the allocated STAKE_SHARE_PRICE_GUARANTEE_FUND.
        let stake_amount = self.staked_amount_from_num_shares_rounded_up(num_shares);

        self.total_staked_balance += stake_amount;
        self.total_stake_shares += num_shares;

        env::log(
            format!(
                "@{} staking {}. Received {} new staking shares. Total {} unstaked balance and {} staking shares",
                account.account_id, charge_amount, num_shares, account.unstaked_balance, account.stake_shares
            )
                .as_bytes(),
        );
        env::log(
            format!(
                "Contract total staked balance is {}. Total number of shares {}",
                self.total_staked_balance, self.total_stake_shares
            )
            .as_bytes(),
        );
    } 

    pub(crate) fn internal_add_tickets(&mut self, amount: u128, multiplier: u64) {
        assert!(amount > 0, "Staking amount should be positive");
        
        let tickets_amount = amount / MIN_TICKET_DEPOSIT_PRICE;
        let mut tickets_num = tickets_amount as u64;
        

        let mut account = self.get_account(env::predecessor_account_id());
        tickets_num = tickets_num * multiplier;
        account.tickets_amount += tickets_num;
        self.save_account(&account);
        println!("tickets added: {}", tickets_num);
        
        let next_tickets_count = self.tickets_count + tickets_num;
        let mut optional = Some(self.tickets_count);

        while let Some(i) = optional {
            if i >= next_tickets_count {
                optional = None;
            } else {
                self.tickets_random_slots.push(&account.account_id);
                optional = Some(i + 1);
            }
        }
        self.tickets_count = next_tickets_count;
    }

    pub(crate) fn internal_remove_tickets(&mut self, amount: u128) {
        assert!(amount > 0, "Staking amount should be positive");
        
        let tickets_amount = amount / MIN_TICKET_DEPOSIT_PRICE;
        let mut tickets_num = tickets_amount as u64;

        let mut account = self.get_account(env::predecessor_account_id());
        tickets_num = tickets_num * account.tickets_multiplier;
        account.tickets_amount -= tickets_num;
        account.tickets_multiplier = 1;
        self.save_account(&account);

        println!("tickets removed: {}", tickets_amount);

        let next_tickets_count = self.tickets_count - tickets_num;

        for i in 0..next_tickets_count {
            let ticket_owner = self.tickets_random_slots.get(i as u64).unwrap();
            if ticket_owner == account.account_id && tickets_num > 0 {
                self.tickets_random_slots.swap_remove(i as u64);
                tickets_num -= 1;
            }
        }

        self.tickets_count = next_tickets_count;
    }

    pub(crate) fn internal_unstake(&mut self, amount: u128) {
        assert!(amount > 0, "Unstaking amount should be positive");

        let account_id = env::predecessor_account_id();
        let mut account = self.get_account(account_id);

        assert!(
            self.total_staked_balance > 0,
            "The contract doesn't have staked balance"
        );
        // Calculate the number of shares required to unstake the given amount.
        // NOTE: The number of shares the account will pay is rounded up.
        let num_shares = self.num_shares_from_staked_amount_rounded_up(amount);
        assert!(
            num_shares > 0,
            "Invariant violation. The calculated number of \"stake\" shares for unstaking should be positive"
        );
        assert!(
            account.stake_shares >= num_shares,
            "Not enough staked balance to unstake"
        );

        // Calculating the amount of tokens the account will receive by unstaking the corresponding
        // number of "stake" shares, rounding up.
        let receive_amount = self.staked_amount_from_num_shares_rounded_up(num_shares);
        assert!(
            receive_amount > 0,
            "Invariant violation. Calculated staked amount must be positive, because \"stake\" share price should be at least 1"
        );

        account.stake_shares -= num_shares;
        account.unstaked_balance += receive_amount;
        account.stake_points -= 1;
        account.unstaked_available_epoch_height = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK;
        self.save_account(&account);

        // The amount tokens that will be unstaked from the total to guarantee the "stake" share
        // price never decreases. The difference between `receive_amount` and `unstake_amount` is
        // paid from the allocated STAKE_SHARE_PRICE_GUARANTEE_FUND.
        let unstake_amount = self.staked_amount_from_num_shares_rounded_down(num_shares);

        self.total_staked_balance -= unstake_amount;
        self.total_stake_shares -= num_shares;

        env::log(
            format!(
                "@{} unstaking {}. Spent {} staking shares. Total {} unstaked balance and {} staking shares",
                account.account_id, receive_amount, num_shares, account.unstaked_balance, account.stake_shares
            )
                .as_bytes(),
        );
        env::log(
            format!(
                "Contract total staked balance is {}. Total number of shares {}",
                self.total_staked_balance, self.total_stake_shares
            )
            .as_bytes(),
        );
    }

    pub(crate) fn internal_restake(&mut self) {
        if self.is_restake_paused {
            return;
        }
        // Stakes with the staking public key. If the public key is invalid the entire function
        // call will be rolled back.
        Promise::new(env::current_account_id())
            .stake(self.total_staked_balance, self.stake_public_key.clone())
            .then(ext_self::on_stake_action(
                &env::current_account_id(),
                NO_DEPOSIT,
                ON_STAKE_ACTION_GAS,
            ));
    }

    pub(crate) fn num_shares_from_staked_amount_rounded_down(
        &self,
        amount: Balance,
    ) -> Balance {
        assert!(
            self.total_staked_balance > 0,
            "The total staked balance can't be 0"
        );
        (U256::from(self.total_stake_shares) * U256::from(amount)
            / U256::from(self.total_staked_balance))
        .as_u128()
    }

    /// Returns the number of "stake" shares rounded up corresponding to the given staked balance
    /// amount.
    ///
    /// Rounding up division of `a / b` is done using `(a + b - 1) / b`.
    pub(crate) fn num_shares_from_staked_amount_rounded_up(
        &self,
        amount: Balance,
    ) -> Balance {
        assert!(
            self.total_staked_balance > 0,
            "The total staked balance can't be 0"
        );
        ((U256::from(self.total_stake_shares) * U256::from(amount)
            + U256::from(self.total_staked_balance - 1))
            / U256::from(self.total_staked_balance))
        .as_u128()
    }

    /// Returns the staked amount rounded down corresponding to the given number of "stake" shares.
    pub(crate) fn staked_amount_from_num_shares_rounded_down(
        &self,
        num_shares: Balance,
    ) -> Balance {
        assert!(
            self.total_stake_shares > 0,
            "The total number of stake shares can't be 0"
        );
        (U256::from(self.total_staked_balance) * U256::from(num_shares)
            / U256::from(self.total_stake_shares))
        .as_u128()
    }

    /// Returns the staked amount rounded up corresponding to the given number of "stake" shares.
    ///
    /// Rounding up division of `a / b` is done using `(a + b - 1) / b`.
    pub(crate) fn staked_amount_from_num_shares_rounded_up(
        &self,
        num_shares: Balance,
    ) -> Balance {
        assert!(
            self.total_stake_shares > 0,
            "The total number of stake shares can't be 0"
        );
        ((U256::from(self.total_staked_balance) * U256::from(num_shares)
            + U256::from(self.total_stake_shares - 1))
            / U256::from(self.total_stake_shares))
        .as_u128()
    }

    // pub(crate) fn random_winners(&mut self) {
    //     self.random_u64();
    // }
    pub(crate) fn get_prize_winner(&mut self) -> AccountId {
        assert!(self.next_prize_event_epoch_height <= env::epoch_height(), "Next prize event time not reached");
        let ticket_id = self.random_u64(0, self.tickets_count);
        println!("Winning Ticket: {}", ticket_id);
        let winner = self.tickets_random_slots.get(ticket_id as u64).unwrap();
        winner
    }

    pub(crate) fn transfer_prize_to_winner(&mut self, winner: AccountId) -> Promise {
        assert!(self.next_prize_event_epoch_height <= env::epoch_height(), "Next prize event time not reached");
        let prize = self.total_reward;
        self.total_reward = 0;
        Promise::new(winner).transfer(prize)
    }

    pub(crate) fn random_u64(&self, min_inc: u64, max_exc: u64) -> u64 {
        // Returns a random number between min (included) and max (excluded)
        let seed_vec = env::random_seed();
        let mut seed: <ChaCha8Rng as SeedableRng>::Seed = env::sha256(&seed_vec.to_owned()).try_into().unwrap();
        thread_rng().fill(&mut seed);
        let mut rng = ChaCha8Rng::from_seed(seed);
        let random = rng.next_u64() % (max_exc - min_inc) + min_inc;
        random
    }

    pub(crate) fn validate_nft_owner(&mut self) -> Promise {
        ext_nft_enumeration::nft_tokens_for_owner(
            env::predecessor_account_id(),
            None,
            None,
            &self.nft_contract_id,
            0,
            5_000_000_000_000
        )
        .then(ext_self::nft_valid_callback(
            &env::current_account_id(), // this contract's account id
            0, // yocto NEAR to attach to the callback
            5_000_000_000_000 // gas to attach to the callback
        ))
    }

    pub fn nft_valid_callback(&self) -> bool {
        assert_eq!(
            env::promise_results_count(),
            1,
            "This is a callback method"
        );
      
        // handle the result from the cross contract call this method is a callback for
        match env::promise_result(0) {
          PromiseResult::NotReady => unreachable!(),
          PromiseResult::Failed => false,
          PromiseResult::Successful(result) => {
              let tokens = near_sdk::serde_json::from_slice::<Vec<Token>>(&result).unwrap();
              if tokens.len() > 0 {
                  true
              } else {
                  false
              }
          },
        }
    }

    pub fn pay_nft_for_multiplier(&mut self, token: Token) -> Promise {
        ext_nft::nft_transfer_call(
            env::current_account_id(),
            token.clone().token_id,
            None,
            None,
            "Transfered".to_owned(),
            &self.nft_contract_id,
            0,
            5_000_000_000_000
        )
        .then(ext_self::nft_transfer_callback(
            token.token_id,
            &env::current_account_id(), // this contract's account id
            0, // yocto NEAR to attach to the callback
            5_000_000_000_000 // gas to attach to the callback
        ))
    }

    pub fn nft_transfer_callback(&mut self, token_id: TokenId) -> bool {
        assert_eq!(
            env::promise_results_count(),
            1,
            "This is a callback method"
        );
      
        // handle the result from the cross contract call this method is a callback for
        match env::promise_result(0) {
          PromiseResult::NotReady => unreachable!(),
          PromiseResult::Failed => false,
          PromiseResult::Successful(result) => {
              let is_transfer_successful = near_sdk::serde_json::from_slice::<bool>(&result).unwrap();
              if is_transfer_successful {          
                    let mut account = self.get_account(env::predecessor_account_id());
                    account.tickets_multiplier = 2;
                    let current_staked_amount = account.staked_balance;

                    self.internal_add_tickets(current_staked_amount, account.tickets_multiplier - 1);
                    self.nft_hold.push(&token_id);
              }
              is_transfer_successful
          },
        }
    }

    pub fn claim_reward_nft(&mut self) -> Promise {
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.stake_points >= 10, "Not enough Stake Times to Claim NFT");
        account.stake_points = 0;
        let token_id = self.nft_hold.pop().unwrap();

        ext_nft::nft_transfer_call(
            env::predecessor_account_id(),
            token_id.clone(),
            None,
            None,
            "Transfered".to_owned(),
            &self.nft_contract_id,
            0,
            5_000_000_000_000
        )
        // .then(ext_self::nft_transfer_callback(
        //     token_id,
        //     &env::current_account_id(), // this contract's account id
        //     0, // yocto NEAR to attach to the callback
        //     5_000_000_000_000 // gas to attach to the callback
        // ))
    }
}

#[ext_contract(ext_nft)]
trait NonFungibleToken {
    // change methods
    fn nft_transfer(&mut self, receiver_id: String, token_id: String, approval_id: Option<u64>, memo: Option<String>);
    fn nft_transfer_call(&mut self, receiver_id: String, token_id: String, approval_id: Option<u64>, memo: Option<String>, msg: String) -> bool;

    // view method
    fn nft_token(&self, token_id: String) -> Option<Token>;
}

#[ext_contract(ext_nft_enumeration)]
trait NonFungibleTokenApprovalManagement: NonFungibleToken {
    fn nft_total_supply(&self) -> U128;
    fn nft_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token>;
    fn nft_supply_for_owner(&self, account_id: String) -> String;
    fn nft_tokens_for_owner(&self, account_id: String, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token>;
}