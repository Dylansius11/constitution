use super::*;
use crate::GateExt;

#[near]
impl Gate {
    #[payable]
    pub fn deposit_credit(&mut self, account: Option<AccountId>) {
        let amount = env::attached_deposit().as_yoctonear();
        check(amount > 0, GateError::ZeroDeposit, "attach credit");
        let account = account.unwrap_or_else(env::predecessor_account_id);
        let balance = self.credit.get(&account).copied().unwrap_or(0);
        let balance = balance
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str(GateError::InsufficientCredit.code()));
        self.total_liabilities = self
            .total_liabilities
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str(GateError::InsufficientCredit.code()));
        self.credit.insert(account, balance);
        self.assert_solvent(0);
    }

    pub fn withdraw_credit(&mut self, amount: U128) -> Promise {
        let account = env::predecessor_account_id();
        let balance = self.credit.get(&account).copied().unwrap_or(0);
        check(
            balance >= amount.0,
            GateError::InsufficientCredit,
            "unreserved balance",
        );
        self.credit.insert(account.clone(), balance - amount.0);
        self.total_liabilities -= amount.0;
        self.pending_withdrawals = self
            .pending_withdrawals
            .checked_add(amount.0)
            .unwrap_or_else(|| env::panic_str(GateError::InsufficientCredit.code()));
        self.assert_solvent(amount.0);
        Promise::new(account.clone())
            .transfer(NearToken::from_yoctonear(amount.0))
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(near_sdk::Gas::from_tgas(10))
                    .on_withdraw_credit(account, amount),
            )
    }

    #[private]
    pub fn on_withdraw_credit(&mut self, account: AccountId, amount: U128) -> bool {
        let Some(pending) = self.pending_withdrawals.checked_sub(amount.0) else {
            env::log_str("Withdrawal callback has no matching reserve");
            return false;
        };
        if env::promise_results_count() == 1 && env::promise_result_checked(0, 0).is_ok() {
            self.pending_withdrawals = pending;
            return true;
        }
        // A failed transfer is refunded by the runtime before this callback.
        // The existing credit entry is retained while the transfer is in flight.
        let credit = self.credit.get(&account).copied().unwrap_or(0);
        let (Some(credit), Some(liabilities)) = (
            credit.checked_add(amount.0),
            self.total_liabilities.checked_add(amount.0),
        ) else {
            env::log_str("Withdrawal refund accounting overflow");
            return false;
        };
        self.credit.insert(account, credit);
        self.total_liabilities = liabilities;
        self.pending_withdrawals = pending;
        false
    }
}

impl Gate {
    /// Shared terminal transition hook. Signing/verdict paths must call it exactly once.
    pub(crate) fn release_reservation(&mut self, proposal: &mut Proposal) {
        check(
            self.try_release_reservation(proposal),
            GateError::InsufficientCredit,
            "reservation release",
        );
    }

    pub(crate) fn try_release_reservation(&mut self, proposal: &mut Proposal) -> bool {
        // An old fee-bump callback must never clear a newer proposal's active slot.
        if self.active_by_account.get(&proposal.proposer) != Some(&proposal.id) {
            return proposal.reserved == 0;
        }
        let balance = self.credit.get(&proposal.proposer).copied().unwrap_or(0);
        let Some(balance) = balance.checked_add(proposal.reserved) else {
            return false;
        };
        self.credit.insert(proposal.proposer.clone(), balance);
        proposal.reserved = 0;
        self.active_by_account.remove(&proposal.proposer);
        self.active_by_key.remove(&active_key(
            &proposal.proposer,
            &proposal.new_impl_codehash,
            &proposal.init_calldata,
        ));
        true
    }
}

pub(crate) fn active_key(proposer: &AccountId, codehash: &[u8; 32], calldata: &[u8]) -> [u8; 32] {
    // One-byte length frames the variable account id (NEAR maximum 64 bytes).
    let account = proposer.as_bytes();
    let mut preimage = [0u8; 1 + 64 + 32 + MAX_INIT_CALLDATA];
    preimage[0] = account.len() as u8;
    preimage[1..1 + account.len()].copy_from_slice(account);
    let offset = 1 + account.len();
    preimage[offset..offset + 32].copy_from_slice(codehash);
    preimage[offset + 32..offset + 32 + calldata.len()].copy_from_slice(calldata);
    env::sha256_array(&preimage[..offset + 32 + calldata.len()])
}
