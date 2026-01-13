use std::fs::OpenOptions;
use std::sync::Arc;
use std::io::Write;
use anyhow::Result;
use chromiumoxide::Page;
use futures::stream::{self, StreamExt};
use tokio::time::{Duration, sleep};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::app::state::AppState;
use crate::app::models::Paper;
use crate::app::types::{ProcessResult, ProcessStats};
use crate::config::AppConfig;

use super::processors::{catalogue, paper};


/// 运行试卷处理流程
pub async fn run(state: &AppState, app_config: AppConfig) -> Result<()> {
    info!("🚀 开始试卷下载流程...");
    info!("📊 页面范围: {} - {}", app_config.start_page, app_config.end_page);

    let mut total = ProcessStats::default();

    for page_num in app_config.start_page..app_config.end_page {
        // 记录进度
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("other/proceed.txt")?;
        writeln!(file, "{}", page_num)?;
        
        // 处理目录页
        match catalogue::process_catalogue_page(page_num, &state.browser).await {
            Ok(papers) => {
                if papers.is_empty() {
                    debug!("页面 {} 没有试卷，跳过", page_num);
                    continue;
                }

                // 检查试卷是否已存在
                let (stats, pending) = check_papers_existence(papers, Arc::clone(&state.page)).await;

                // 下载并处理待处理的试卷
                let stats_after_dl = download_papers(
                    pending,
                    state,
                    &app_config.output_dir,
                    app_config.concurrency,
                    stats,
                )
                .await;

                // 更新总统计
                total.success += stats_after_dl.success;
                total.exists += stats_after_dl.exists;
                total.failed += stats_after_dl.failed;
                
                info!(
                    "✅ 页面 {} 完成: 成功 {}，已存在 {}，失败 {}",
                    page_num, stats_after_dl.success, stats_after_dl.exists, stats_after_dl.failed
                );
            }
            Err(e) => {
                warn!("❌ 页面 {} 失败: {}", page_num, e);
            }
        }

        sleep(Duration::from_millis(app_config.delay_ms)).await;
    }

    info!("🎉 流程完成！");
    info!(
        "📊 统计: 成功 {} 个，已存在 {} 个，失败 {} 个",
        total.success, total.exists, total.failed
    );

    Ok(())
}

/// 检查试卷是否已存在，返回统计信息和待处理列表
async fn check_papers_existence(
    papers: Vec<Paper>,
    tiku_page: Arc<RwLock<Page>>,
) -> (ProcessStats, Vec<Paper>) {
    stream::iter(papers.into_iter())
        .then(|mut paper| {
            let tiku_page = Arc::clone(&tiku_page);
            async move {
                let page = tiku_page.read().await;
                match paper.check_paper_existence(&*page).await {
                    Ok(true) => (ProcessResult::AlreadyExists, None),
                    Ok(false) => (ProcessResult::Success, Some(paper)),
                    Err(e) => {
                        warn!("❌ 目录页检查失败 '{}': {}", paper.name, e);
                        (ProcessResult::Failed, None)
                    }
                }
            }
        })
        .fold(
            (ProcessStats::default(), Vec::new()),
            |(mut stats, mut keep), (check_result, paper_opt)| async move {
                match check_result {
                    ProcessResult::AlreadyExists => stats.add_result(&ProcessResult::AlreadyExists),
                    ProcessResult::Failed => stats.add_result(&ProcessResult::Failed),
                    ProcessResult::Success => {
                        if let Some(p) = paper_opt {
                            keep.push(p);
                        }
                    }
                }
                (stats, keep)
            },
        )
        .await
}

/// 下载并处理试卷列表
async fn download_papers(
    papers: Vec<Paper>,
    state: &AppState,
    output_dir: &str,
    concurrency: usize,
    initial_stats: ProcessStats,
) -> ProcessStats {
    stream::iter(papers.into_iter().map(|paper| {
        let browser = Arc::clone(&state.browser);
        let output_dir = output_dir.to_string();
        let page_handle = Arc::clone(&state.page);
        
        async move {
            let res = paper::process_single_paper(&paper, &browser, &output_dir, page_handle).await;
            (paper.name, res)
        }
    }))
    .buffer_unordered(concurrency)
    .fold(initial_stats, |mut stats, (title, result)| async move {
        match result {
            Ok(ProcessResult::Success) => stats.add_result(&ProcessResult::Success),
            Ok(ProcessResult::AlreadyExists) => stats.add_result(&ProcessResult::AlreadyExists),
            Ok(ProcessResult::Failed) => {
                warn!("❌ 处理失败: {}", title);
                stats.add_result(&ProcessResult::Failed);
            }
            Err(e) => {
                warn!("❌ 处理 '{}' 时出错: {}", title, e);
                stats.add_result(&ProcessResult::Failed);
            }
        }
        stats
    })
    .await
}

