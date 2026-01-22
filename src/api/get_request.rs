use anyhow::Result;
use reqwest::Client;
use reqwest::header::{ACCEPT, COOKIE, HOST, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use serde_json::Value;
use tracing::{debug, info};

use crate::config::get;

pub async fn send_api_get_request(url: &str) -> Result<Value> {
    let mut headers = HeaderMap::new();
    // 1. 模拟浏览器 UA
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0"));

    // 2. 防盗链 Referer
    headers.insert(REFERER, HeaderValue::from_static("https://tk-lpzx.xdf.cn/"));

    // 3. 跨域 Origin
    headers.insert(ORIGIN, HeaderValue::from_static("https://tk-lpzx.xdf.cn"));

    // 4. Host
    headers.insert(HOST, HeaderValue::from_static("tps-tiku-api.staff.xdf.cn"));

    // 5. Accept
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    let token = &get().token;
    // 6. 【最关键】Cookie (直接复制抓包里的完整字符串)
    // 包含: XDFUUID, e2e, e2mf, token
    headers.insert(COOKIE, HeaderValue::from_str(token)?);

    headers.insert(
        "tikutoken",
        HeaderValue::from_static("732FD8402F95087CD934374135C46EE5"),
    );

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let resp = client.get(url).headers(headers).send().await?;

    // 检查 HTTP 状态码
    let status = resp.status();
    info!("API GET 响应状态码: {}", status);

    let resp_json: Value = resp.json().await?;

    debug!(
        "API GET 响应 JSON: {}",
        serde_json::to_string_pretty(&resp_json).unwrap_or_default()
    );

    // 检查响应是否成功
    if !status.is_success() {
        let error_msg = serde_json::to_string(&resp_json).unwrap_or_default();
        return Err(anyhow::anyhow!(
            "API GET 请求失败，状态码: {}。响应: {}",
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
    use tokio;

    #[tokio::test]
    async fn test_send_api_get_request() {
        logger::init_test();
        let url = "https://tps-tiku-api.staff.xdf.cn/paper/check/paperName?paperName=测试试卷&operationType=1&paperId=";
        let result = send_api_get_request(url).await;
        println!("API GET 请求结果: {:?}", result);
        assert!(result.is_ok());
    }
}
