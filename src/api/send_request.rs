use anyhow::Result;
use reqwest::Client;
use reqwest::header::{
    ACCEPT, CONTENT_TYPE, COOKIE, HOST, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT,
};
use serde_json::Value;
use tracing::{debug, info};
use crate::config;

pub async fn send_api_request(url: &str, playload: &Value) -> Result<Value> {
    // let url = "https://tps-tiku-api.staff.xdf.cn/paper/new/save";

    let mut headers = HeaderMap::new();
    // 1. 模拟浏览器 UA
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0"));

    // 2. 防盗链 Referer
    headers.insert(REFERER, HeaderValue::from_static("https://tk-lpzx.xdf.cn/"));

    // 3. 跨域 Origin
    headers.insert(ORIGIN, HeaderValue::from_static("https://tk-lpzx.xdf.cn"));

    // 4. Host
    headers.insert(HOST, HeaderValue::from_static("tps-tiku-api.staff.xdf.cn"));

    // 5. 内容类型
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );

    // 6. 【最关键】Cookie (从配置文件读取)
    // 包含: e2e, e2mf, token
    let cookie_value = &config::get().token;
    headers.insert(COOKIE, HeaderValue::from_str(cookie_value)?);

    headers.insert(
        "tikutoken",
        HeaderValue::from_static("732FD8402F95087CD934374135C46EE5"),
    );

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let payload_str = serde_json::to_string(&playload)?;

    let resp = client
        .post(url)
        .headers(headers)
        .body(payload_str)
        .send()
        .await?;

    // 检查 HTTP 状态码
    let status = resp.status();
    info!("API 响应状态码: {}", status);

    let resp_json: Value = resp.json().await?;

    debug!(
        "API 响应 JSON: {}",
        serde_json::to_string_pretty(&resp_json).unwrap_or_default()
    );

    // 检查响应是否成功
    if !status.is_success() {
        let error_msg = serde_json::to_string(&resp_json).unwrap_or_default();
        return Err(anyhow::anyhow!(
            "API 请求失败，状态码: {}。响应: {}",
            status,
            error_msg
        ));
    }

    // 检查响应是否成功（根据 code 或 success 字段）
    let _is_success = resp_json
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || resp_json
            .get("code")
            .and_then(|v| v.as_u64())
            .map(|c| c == 200)
            .unwrap_or(false);

    Ok(resp_json)
}

#[cfg(test)]
mod tests {
    use crate::app::logger;

    use super::*;
    use serde_json::json;
    use tokio;
    #[tokio::test]
    async fn test_send_api_request() {
        logger::init_test();
        let payload = json!({
          "stage": "3",
          "subject": "61",
          "imagePath": "https://k12static.xdf.cn/k12-paperxdfUploadtikuImageDir/1-1768265578954.png",
          "text": "A、已号会人工服火"
        });
        let result = send_api_request(
            "https://tps-tiku-api.staff.xdf.cn/api/third/xkw/question/v2/text-search",
            &payload,
        )
        .await;
        assert!(result.is_ok());
    }
}
