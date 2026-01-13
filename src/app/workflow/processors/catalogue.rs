use anyhow::Result;
use chromiumoxide::Browser;
use tracing::{debug, info, warn};

use crate::app::models::Paper;
use crate::modules::browser::get_or_open_page;
use crate::modules::catalogue::fetch_paper_list;

/// 处理目录页，返回试卷列表
pub async fn process_catalogue_page(
    page_number: i32,
    browser: &Browser,
) -> Result<Vec<Paper>> {
    let catalogue_url = format!("https://zujuan.xkw.com/czls/shijuan/bk/p{}", page_number);
    info!("📖 正在处理目录页 {}...", page_number);

    let catalogue_page = get_or_open_page(browser, &catalogue_url, None).await?;

    let result = async {
        let papers = fetch_paper_list(&catalogue_page).await?;
        info!("📄 在页面 {} 找到 {} 个试卷", page_number, papers.len());
        Ok(papers)
    }
    .await;

    debug!("正在关闭目录页");
    if let Err(e) = catalogue_page.close().await {
        warn!("关闭目录页失败: {}，但继续处理", e);
    }
    result
}
