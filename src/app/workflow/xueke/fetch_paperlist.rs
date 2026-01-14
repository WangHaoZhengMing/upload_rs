use chromiumoxide::Browser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error};
use anyhow::anyhow;


pub async fn fetch_paper_list(browser: &Browser, base_url: &str, page_number: i32) -> anyhow::Result<Vec<PaperInfo>> {
    let page_url = format!("{}p{}", base_url, page_number);
    debug!("正在打开目录页: {}", page_url);
    
    let page = browser.new_page(&page_url).await?;
    
    let js_code = r#"
        () => {
            const elements = document.querySelectorAll("div.info-item.exam-info a.exam-name");
            return Array.from(elements).map(el => ({
                url: 'https://zujuan.xkw.com' + el.getAttribute('href'),
                title: el.innerText.trim()
            }));
        }
    "#;

    debug!("正在获取目录页的试卷列表");
    let response: Value = page
        .evaluate(js_code)
        .await
        .map_err(|e| {
            error!("执行获取试卷列表脚本失败: {}", e);
            e
        })?
        .into_value()
        .map_err(|e| {
            error!("获取试卷列表结果失败: {}", e);
            anyhow!("获取试卷列表结果失败: {}", e)
        })?;

    let papers: Vec<PaperInfo> = serde_json::from_value(response).map_err(|e| {
        error!("解析试卷列表失败: {}", e);
        anyhow!("解析试卷列表失败: {}", e)
    })?;
    debug!("成功获取到 {} 个试卷", papers.len());

    // 关闭页面释放资源
    if let Err(e) = page.close().await {
        debug!("关闭目录页失败: {}", e);
    }

    Ok(papers)
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperInfo {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub is_exit: bool,
}