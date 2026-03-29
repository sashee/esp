use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SIMULATOR_RUN_KIND: &str = "simulator-run";
pub const SIMULATOR_RUN_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunDocument {
    pub kind: String,
    pub version: u32,
    #[serde(default)]
    pub items: Vec<Value>,
}

impl Default for RunDocument {
    fn default() -> Self {
        Self {
            kind: SIMULATOR_RUN_KIND.to_string(),
            version: SIMULATOR_RUN_VERSION,
            items: Vec::new(),
        }
    }
}

impl RunDocument {
    pub fn is_simulator_run(&self) -> bool {
        self.kind == SIMULATOR_RUN_KIND
    }
}
