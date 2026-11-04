use super::*;
use crate::GateExt;

#[near]
impl Gate {
    pub fn is_signing_paused(&self) -> bool {
        self.signing_paused
    }
    pub fn get_total_liabilities(&self) -> U128 {
        U128(self.total_liabilities)
    }
    /// Yocto that must be attached to `propose`, exactly. Non-refundable; it funds the record.
    pub fn get_proposal_fee(&self) -> U128 {
        U128(Gate::proposal_storage_fee())
    }
    /// Credit an account must hold before `propose` (the refundable reservation).
    pub fn get_required_credit(&self) -> U128 {
        U128(self.constitution.params.proposal_deposit)
    }
    /// Yocto taken from the first deposit of a new credit account to pay for its entry.
    pub fn get_credit_entry_fee(&self, account: AccountId) -> U128 {
        U128(Gate::credit_entry_fee(&account))
    }
    /// Liquid balance after storage and after every user liability; 0 when exhausted.
    pub fn get_operating_surplus(&self) -> U128 {
        U128(self.operating_surplus())
    }
    pub fn get_config(&self) -> &Config {
        &self.config
    }
    pub fn get_constitution(&self) -> &Constitution {
        &self.constitution
    }
    pub fn get_roles(&self) -> &Roles {
        &self.roles
    }
    pub fn list_verifiers(
        &self,
        from_index: u32,
        limit: u32,
    ) -> Vec<(Base64<32>, &VerifierRecord)> {
        self.verifiers
            .iter()
            .skip(from_index as usize)
            .take(limit.min(50) as usize)
            .map(|(key, record)| (Base64(*key), record))
            .collect()
    }
    pub fn get_proposal(&self, id: u64) -> Option<&Proposal> {
        self.proposals.get(&id)
    }
    pub fn list_proposals(&self, from_id: u64, limit: u32) -> Vec<&Proposal> {
        (from_id..self.next_proposal_id)
            .take(limit.min(50) as usize)
            .filter_map(|id| self.proposals.get(&id))
            .collect()
    }
    pub fn get_change(&self, id: u64) -> Option<&PendingChange> {
        self.changes.get(&id)
    }
    pub fn list_changes(&self, from_id: u64, limit: u32) -> Vec<&PendingChange> {
        // Terminal changes are deleted, so ids are sparse: walk the live entries, not an id range.
        let mut live: Vec<&PendingChange> = self
            .changes
            .iter()
            .filter(|(id, _)| **id >= from_id)
            .map(|(_, change)| change)
            .collect();
        live.sort_by_key(|change| change.id);
        live.truncate(limit.min(50) as usize);
        live
    }
    pub fn get_credit(&self, account: AccountId) -> U128 {
        U128(self.credit.get(&account).copied().unwrap_or(0))
    }
    pub fn get_active(&self, account: AccountId) -> Option<u64> {
        self.active_by_account.get(&account).copied()
    }
}
