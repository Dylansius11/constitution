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
        self.credit.insert(account, balance);
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
        Promise::new(account).transfer(NearToken::from_yoctonear(amount.0))
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
            &proposal.new_impl_codehash,
            &proposal.init_calldata,
        ));
        true
    }
}

pub(crate) fn active_key(codehash: &[u8; 32], calldata: &[u8]) -> [u8; 32] {
    // Calldata is validated before hashing. Fixed buffer avoids a heap allocation.
    let mut preimage = [0u8; 32 + MAX_INIT_CALLDATA];
    preimage[..32].copy_from_slice(codehash);
    preimage[32..32 + calldata.len()].copy_from_slice(calldata);
    env::sha256_array(&preimage[..32 + calldata.len()])
}
