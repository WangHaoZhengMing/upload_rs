use anyhow::Result;
use serde_json::Value;
use super::send_request::send_api_request;

const SAVE_PAPER_URL: &str = "https://tps-tiku-api.staff.xdf.cn/paper/new/save";

pub async fn submit_paper_api(payload: &Value) -> Result<Value> {
    send_api_request(SAVE_PAPER_URL, payload).await
}

