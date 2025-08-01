use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct TestRequest {
    pub hello: String,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct TestResponse {
    pub world: String,
}
