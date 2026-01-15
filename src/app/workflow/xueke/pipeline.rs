use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::fs;
use tracing::{debug, info, warn};

use crate::api::check_paper_exit::check_paper_name_exist;
use crate::app::state::AppState;
use crate::app::workflow::xueke::download::download_paper;
use crate::app::workflow::xueke::fetch_paperlist::fetch_paper_list;
use crate::config;

pub async fn run_xueke_pipeline(
    state: AppState,
    page_num: i32,
) -> Result<Vec<crate::app::models::Paper>> {
    let browser = state.browser.clone();
    let app_config = config::get();

    info!("开始处理第 {} 页", page_num);

    // 获取该页的试卷列表
    let paper_list =
        match fetch_paper_list(&browser, &app_config.catalogue_base_url, page_num).await {
            Ok(list) => list,
            Err(e) => {
                warn!("获取第 {} 页试卷列表失败: {}", page_num, e);
                return Ok(vec![]);
            }
        };

    info!("第 {} 页找到 {} 个试卷", page_num, paper_list.len());

    // 并发检查所有试卷是否已存在
    let paper_list = stream::iter(paper_list.into_iter())
        .map(|mut paper| async move {
            match check_paper_name_exist(&paper.title, 1, None).await {
                Ok(exists) => {
                    paper.is_exit = exists;
                    if exists {
                        info!("试卷已存在，将跳过: {}", paper.title);
                    }
                }
                Err(e) => {
                    warn!("检查试卷 '{}' 是否存在时出错: {}", paper.title, e);
                    paper.is_exit = false; // 出错时默认不存在，尝试下载
                }
            }
            paper
        })
        .buffer_unordered(10)
        .collect::<Vec<_>>()
        .await;

    // 过滤并收集不存在的试卷
    let papers_to_download: Vec<_> = paper_list.into_iter().filter(|p| !p.is_exit).collect();

    info!(
        "第 {} 页需下载 {} 个试卷",
        page_num,
        papers_to_download.len()
    );

    // 创建输出目录
    fs::create_dir_all(&app_config.output_dir)?;

    // 并发下载所有试卷（最多2个并发）
    let total = papers_to_download.len();
    let downloaded_papers = stream::iter(papers_to_download.into_iter().enumerate())
        .map(|(idx, paper_info)| {
            let state = state.clone();
            let output_dir = app_config.output_dir.clone();
            async move {
                info!("[{}/{}] 开始下载: {}", idx + 1, total, paper_info.title);

                match download_paper(&state, &paper_info.url).await {
                    Ok(paper) => {
                        info!("✅ 成功处理试卷: {}", paper.name);

                        // 仅在 debug 模式下将 paper 保存到 TOML 文件
                        #[cfg(debug_assertions)]
                        {
                            use std::path::PathBuf;
                            let safe_name = paper
                                .name
                                .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
                            let toml_path =
                                PathBuf::from(&output_dir).join(format!("{}.toml", safe_name));
                            match toml::to_string_pretty(&paper) {
                                Ok(toml_str) => {
                                    if let Err(e) = fs::write(&toml_path, toml_str) {
                                        debug!("保存 TOML 文件失败: {:?}, 错误: {}", toml_path, e);
                                    } else {
                                        debug!("已保存试卷数据到: {:?}", toml_path);
                                    }
                                }
                                Err(e) => {
                                    debug!("序列化 TOML 失败: {}", e);
                                }
                            }
                        }
                        Some(paper)
                    }
                    Err(e) => {
                        warn!("❌ 处理试卷失败: {}，错误: {}", paper_info.title, e);
                        None
                    }
                }
            }
        })
        .buffer_unordered(1)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    info!(
        "第 {} 页处理完成，成功下载 {} 个试卷",
        page_num,
        downloaded_papers.len()
    );
    Ok(downloaded_papers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::logger;

    #[tokio::test]
    async fn test_run_xueke_pipeline() {
        logger::init_test();

        info!("开始测试学科网试卷下载流程");

        // 创建应用状态
        let state = match AppState::new().await {
            Ok(s) => s,
            Err(e) => {
                warn!("创建应用状态失败: {}", e);
                return;
            }
        };

        // 运行流程 (测试第一页)
        match run_xueke_pipeline(state, 1).await {
            Ok(_) => {
                info!("测试完成");
            }
            Err(e) => {
                warn!("流程执行失败: {}", e);
            }
        }
    }
}
//617