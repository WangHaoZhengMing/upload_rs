use anyhow::Result;
use chromiumoxide::{Browser, Page};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::app::models::Paper;
use crate::app::types::ProcessResult;
use crate::modules::browser::{get_or_open_page, download_page};
use crate::modules::storage::persist_paper_locally;
use crate::workflow::upload_to_xueke::save_paper;

/// 处理单个试卷的完整流程
pub async fn process_single_paper(
    paper_info: &Paper,
    browser: &Browser,
    output_dir: &str,
    tiku_page: Arc<RwLock<Page>>,
) -> Result<ProcessResult> {
    let paper_page = get_or_open_page(browser, &paper_info.url, None).await?;

    debug!("开始处理试卷: {}", paper_info.name);
    let result: Result<ProcessResult> = async {
        let mut page_data = download_page(&paper_page).await.map_err(|e| {
            warn!("下载页面数据失败: {}", e);
            e
        })?;
        let page = tiku_page.read().await;
        save_paper((*page).clone(), &mut page_data).await?;
        persist_paper_locally(&page_data, output_dir)?;
        info!("✅ 成功处理: {}", page_data.name);
        Ok(ProcessResult::Success)
    }
    .await;

    debug!("正在关闭试卷页面");
    if let Err(e) = paper_page.close().await {
        warn!("关闭试卷页面失败: {}，但继续处理", e);
    }
    result
}
