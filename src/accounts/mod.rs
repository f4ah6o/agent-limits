pub mod memory;

pub use memory::MemoryStore;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub uuid: String,
    pub email: String,
    pub display_name: String,
    pub rate_limit_tier: String,
    pub last_seen_at: DateTime<Utc>,
    pub raw_blob: Box<RawValue>,
}

pub trait Store: Send + Sync {
    fn list(&self) -> Result<Vec<Account>, String>;
    fn upsert(&self, account: Account) -> Result<(), String>;
}
