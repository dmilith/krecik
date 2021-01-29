use crate::checks::domain::*;
use crate::checks::page::*;
use crate::products::story::*;
use actix::prelude::*;


#[derive(Debug, Clone, Serialize, Deserialize, Default, Message)]
#[rtype(result = "Result<Stories, Stories>")]
/// Generic Check structure:
pub struct Check {
    /// Domains to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domains: Option<Domains>,

    /// Pages to check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<Pages>,

    /// Slack notifier id - taken from config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifier: Option<String>,
}
