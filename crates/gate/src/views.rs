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
        (from_id..self.next_change_id)
            .take(limit.min(50) as usize)
            .filter_map(|id| self.changes.get(&id))
            .collect()
    }
    pub fn get_credit(&self, account: AccountId) -> U128 {
        U128(self.credit.get(&account).copied().unwrap_or(0))
    }
    pub fn get_active(&self, account: AccountId) -> Option<u64> {
        self.active_by_account.get(&account).copied()
    }
}
