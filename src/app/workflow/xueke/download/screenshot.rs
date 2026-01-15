use anyhow::anyhow;
use rayon::prelude::*;
use scraper::{Html, Selector};
use serde_json::Value;
use tracing::debug;

use crate::api::convert_html_to_img::render_question_to_image;
use crate::app::state::AppState;

/// 对每个题目进行截图并返回 base64 编码的列表
pub async fn capture_question_screenshots(
    _state: &AppState,
    styles: &str,
    elements_data: &Value,
) -> anyhow::Result<Vec<String>> {
    let elements_array = elements_data["elements"]
        .as_array()
        .ok_or_else(|| anyhow!("无法获取 elements 数组"))?;

    // 1. 提取所有题目的HTML
    let mut question_htmls = Vec::new();
    for element_obj in elements_array {
        let element_type = element_obj["type"].as_str().unwrap_or("");

        // 跳过标题，只处理题目
        if element_type == "title" {
            continue;
        }

        if element_type == "content" {
            let html_str = element_obj["content"]
                .as_str()
                .ok_or_else(|| anyhow!("无法获取 content 字段"))?;

            let document = Html::parse_document(html_str);
            let exam_item_selector =
                Selector::parse(".exam-item__cnt").map_err(|e| anyhow!("选择器解析失败: {}", e))?;

            for exam_item in document.select(&exam_item_selector) {
                let question_html = exam_item.html();
                question_htmls.push(question_html);
            }
        }
    }

    debug!("提取到 {} 个题目，开始多线程渲染", question_htmls.len());

    // 2. 准备 head 内容（包含样式）
    let head_html = format!(
        r#"
        <meta charset="UTF-8">
        <style>
            {}
            
            body {{
                background-color: #fff;
                padding: 20px;
                margin: 0;
                -webkit-font-smoothing: antialiased;
            }}
            .exam-item__cnt {{ margin: 0 !important; border: none !important; }}
        </style>
        "#,
        styles
    );

    // 3. 使用 rayon 多线程并行渲染所有题目
    let screenshots: Vec<String> = question_htmls
        .par_iter()
        .enumerate()
        .map(|(index, question_html)| {
            // 构造完整的题目HTML（包装在div中）
            let full_html = format!(r#"<div class="exam-item__cnt">{}</div>"#, question_html);

            render_question_to_image(&head_html, &full_html, index)
        })
        .collect::<Result<Vec<_>, _>>()?;

    debug!("所有题目渲染完成");
    Ok(screenshots)
}
