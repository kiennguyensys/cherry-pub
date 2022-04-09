#![allow(dead_code, unused_variables)]
mod local;
mod remote;
pub use local::*;
pub use remote::*;

#[cfg(test)]
mod test {
    use near_sdk_sim::{call, deploy, init_simulator, ContractAccount, UserAccount};
    use super::*;

    // Load in contract bytes
    near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
        // update `contract.wasm` for your contract's name
        LOCAL_WASM_BYTES => "../target/wasm32-unknown-unknown/release/cherry_pool.wasm",
    
        // if you run `cargo build` without `--release` flag:
        REMOTE_WASM_BYTES => "../target/wasm32-unknown-unknown/release/cherry_pub_nft.wasm",
    }

    // init simulated environment
    fn init() -> (
        UserAccount,
        ContractAccount<LocalContract>,
        ContractAccount<RemoteContract>,
    ) {
        // init simulator returns the master account of the simulated environment
        let master_account = init_simulator(None);
        // create account and deploy contract code for LocalContract
        // uses default values for deposit and gas
        let local_contract = deploy!(
            // Contract Proxy
            contract: LocalContract,
            // Contract account id
            contract_id: "local",
            // Bytes of contract
            bytes: &LOCAL_WASM_BYTES,
            // User deploying the contract,
            signer_account: master_account
        );
        // create account and deploy contract code for RemoteContract
        let remote_contract = deploy!(
            // Contract Proxy
            contract: RemoteContract,
            // Contract account id
            contract_id: "remote",
            // Bytes of contract
            bytes: &REMOTE_WASM_BYTES,
            // User deploying the contract,
            signer_account: master_account
        );

        return (master_account, local_contract, remote_contract);
    }

    #[test]
    fn high_level_function_call() {
        let (master_account, local, _remote) = init();
        // simulate a user signing a transaction calling local.xcc
        let res = call!(
            master_account,
            local.xcc("high_fc", "remote", "do_some_work")
        );

        res.assert_success();

        // the result returned by local.xcc
        println!(
            "{:#?}\n{:#?}\n{:#?}\n",
            res,
            res.promise_results(),
            res.unwrap_json::<String>()
        );
    }

    #[test]
    fn high_level_batch_action() {
        let (master_account, local, _remote) = init();
        // simulate a user signing a transaction calling local.xcc
        let res = call!(
            master_account,
            local.xcc("high_ba", "remote", "do_some_work")
        );

        res.assert_success();

        // the result returned by local.xcc
        println!(
            "{:#?}\n{:#?}\n{:#?}\n",
            res,
            res.promise_results(),
            res.unwrap_json::<String>()
        );
    }

    #[test]
    fn low_level_function_call() {
        let (master_account, local, _remote) = init();
        // simulate a user signing a transaction calling local.xcc
        let res = call!(
            master_account,
            local.xcc("low_fc", "remote", "do_some_work")
        );

        res.assert_success();

        // the result returned by local.xcc
        println!(
            "{:#?}\n{:#?}\n{:#?}\n",
            res,
            res.promise_results(),
            res.unwrap_json::<String>()
        );
    }

    #[test]
    fn low_level_batch_action() {
        let (master_account, local, _remote) = init();
        // simulate a user signing a transaction calling local.xcc
        let res = call!(
            master_account,
            local.xcc("low_ba", "remote", "do_some_work")
        );

        res.assert_success();

        // the result returned by local.xcc
        println!(
            "{:#?}\n{:#?}\n{:#?}\n",
            res,
            res.promise_results(),
            res.unwrap_json::<String>()
        );
    }
}