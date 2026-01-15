use anyhow::{anyhow, Ok};
use scraper::{Html, Selector};
use serde_json::Value;
use std::path::Path;
use tokio::fs;
use tracing::{debug, error, info, warn};

use crate::app::models::{Paper, Question};
use crate::app::state::AppState;

use super::extract_meta::extract_paper_metadata;
use super::pdf::generate_pdf;
use super::screenshot::capture_question_screenshots;
use super::scripts::ELEMENTS_DATA_JS;

// 定义接收 JS 返回数据的结构体
#[allow(dead_code)]
#[derive(serde::Deserialize, Debug)]
struct PageData {
    styles: String,
    questions: Vec<String>,
}

pub async fn download_paper(state: &AppState, paper_url: &str) -> anyhow::Result<Paper> {
    let page = state.browser.new_page(paper_url).await?;

    debug!("开始提取页面元素数据");
    let elements_data: Value = page.evaluate(ELEMENTS_DATA_JS).await?.into_value()?;
    debug!("成功获取页面元素数据");

    // 提取页面样式，用于后续截图
    let styles = elements_data["styles"].as_str().unwrap_or("").to_string();

    let elements_array = elements_data["elements"].as_array().ok_or_else(|| {
        error!("无法获取 elements 数组");
        anyhow!("无法获取 elements 数组")
    })?;

    info!("找到 {} 个题目部分。", elements_array.len());

    let mut questions = Vec::new();
    for element_obj in elements_array {
        let element_type = element_obj["type"].as_str().unwrap_or("");

        if element_type == "title" {
            let title = element_obj["title"].as_str().unwrap_or("").to_string();
            if !title.is_empty() {
                debug!("处理章节: {}", title);
                questions.push(Question {
                    origin: String::new(),
                    stem: title,
                    origin_from_our_bank: vec![],
                    is_title: true,
                    imgs: None,
                    screenshot: String::new(), // 标题不需要截图
                });
            }
        } else if element_type == "content" {
            let html_str = element_obj["content"].as_str().ok_or_else(|| {
                error!("无法获取 content 字段");
                anyhow!("无法获取 content 字段")
            })?;

            let document = Html::parse_document(html_str);

            let exam_item_selector =
                Selector::parse(".exam-item__cnt").map_err(|e| anyhow!("选择器解析失败: {}", e))?;
            let origin_selector =
                Selector::parse("a.ques-src").map_err(|e| anyhow!("选择器解析失败: {}", e))?;

            for exam_item in document.select(&exam_item_selector) {
                let stem = exam_item.text().collect::<String>().trim().to_string();

                let img_selector =
                    Selector::parse("img").map_err(|e| anyhow!("图片选择器解析失败: {}", e))?;
                let mut imgs = Vec::new();
                for img in exam_item.select(&img_selector) {
                    if let Some(src) = img.value().attr("src") {
                        imgs.push(src.to_string());
                    }
                    if let Some(data_src) = img.value().attr("data-src") {
                        if !imgs.contains(&data_src.to_string()) {
                            imgs.push(data_src.to_string());
                        }
                    }
                }

                let origin = exam_item
                    .select(&origin_selector)
                    .next()
                    .or_else(|| document.select(&origin_selector).next())
                    .map(|el| el.text().collect::<String>().trim().to_string())
                    .unwrap_or_else(|| "未找到来源".to_string());

                if !stem.is_empty() && stem != "未找到题目" {
                    questions.push(Question {
                        origin,
                        stem,
                        origin_from_our_bank: vec![],
                        is_title: false,
                        imgs: if imgs.is_empty() { None } else { Some(imgs) },
                        screenshot: String::new(), // 后续填充
                    });
                }
            }
        }
    }

    // 提取试卷元数据
    let metadata = extract_paper_metadata(&page).await?;

    debug!("准备生成 PDF 文件");
    let pdf_dir = Path::new("PDF");
    if !pdf_dir.exists() {
        debug!("PDF 目录不存在，正在创建");
        fs::create_dir_all(pdf_dir).await?;
    }
    let pdf_path = pdf_dir.join(format!("{}.pdf", metadata.name_for_pdf));
    debug!("PDF 文件路径: {:?}", pdf_path);

    debug!("开始生成 PDF");
    if let Err(e) = generate_pdf(&page, &pdf_path).await {
        error!("生成 PDF 失败: {}，但继续处理数据", e);
        warn!("生成 PDF 失败: {}，但继续处理数据", e);
    } else {
        info!("已保存 PDF: {:?}", pdf_path);
        debug!("PDF 生成成功");
    }
    // 关闭页面释放资源
    debug!("正在关闭试卷页面");
    if let Err(e) = page.close().await {
        warn!("关闭试卷页面失败: {}", e);
    }
    // ============================================================================
    // 对每个题目进行截图
    // ============================================================================
    debug!("开始对题目进行截图");
    let screenshots = capture_question_screenshots(state, &styles, &elements_data).await?;
    debug!("截图完成，共 {} 张", screenshots.len());

    // 将截图填充到对应的题目中
    let mut screenshot_idx = 0;
    for question in &mut questions {
        if !question.is_title && screenshot_idx < screenshots.len() {
            question.screenshot = screenshots[screenshot_idx].clone();
            screenshot_idx += 1;
        }
    }


    Ok(Paper {
        name: metadata.title,
        province: metadata.province,
        grade: metadata.grade,
        year: metadata.year,
        subject: metadata.subject,
        page_id: None,
        stemlist: questions,
        name_for_pdf: metadata.name_for_pdf,
        url: paper_url.to_string(),
        is_exit: Some(false),
    })
}
