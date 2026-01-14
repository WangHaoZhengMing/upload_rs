use crate::config;
use futures::StreamExt;

use crate::app::{
    state::AppState,
    workflow::{upload_to_xueke::save_paper, xueke::run_xueke_pipeline},
};

pub async fn run(state: AppState) -> anyhow::Result<()> {
    let config = config::get();

    for page_num in config.start_page..=config.end_page {
        tracing::info!(">>> 开始处理第 {} 页", page_num);
        let pages = run_xueke_pipeline(state.clone(), page_num).await?;

        // 提前获取页面句柄，避免循环中重复获取锁
        let tiku_page = state.page.read().await.clone();

        // 并发处理上传任务 (并发度设置为 5)
        futures::stream::iter(pages)
            .map(|mut page| {
                let tiku_page = tiku_page.clone();
                async move {
                    save_paper(tiku_page, &mut page).await
                }
            })
            .buffer_unordered(5)
            .collect::<Vec<anyhow::Result<()>>>()
            .await
            .into_iter()
            .collect::<anyhow::Result<()>>()?;
    }

    Ok(())
}
