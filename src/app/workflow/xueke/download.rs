use anyhow::{Ok, anyhow};
use base64::{Engine as _, engine::general_purpose};
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, PrintToPdfParams};
use scraper::{Html, Selector};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

use crate::app::models::{Paper, Question};
use crate::app::state::AppState;

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

    debug!("正在提取试卷标题");
    let title_value: Value = page.evaluate(TITLE_JS).await?.into_value()?;
    let title: String = title_value.as_str().unwrap_or("未找到标题").to_string();
    debug!("提取到的原始标题: {}", title);

    let title = sanitize_filename(&title);
    debug!("清理后的标题: {}", title);

    debug!("正在提取省份和年级信息");
    let info: Value = page.evaluate(INFO_JS).await?.into_value()?;
    let province = info["shengfen"].as_str().unwrap_or("未找到").to_string();
    let grade = info["nianji"].as_str().unwrap_or("未找到").to_string();
    debug!("省份: {}, 年级: {}", province, grade);

    debug!("正在提取科目信息");
    let subject_value: Value = page.evaluate(SUBJECT_JS).await?.into_value()?;
    let subject_text: String = subject_value.as_str().unwrap_or("未找到科目").to_string();
    debug!("提取到的科目文本: {}", subject_text);

    // 关键词 -> 标准科目名映射
    // 注意：顺序很重要，如果有包含关系（如"道德与法治"和"法治"），长的应该在前
    let subject_mappings = [
        ("语文", "语文"),
        ("数学", "数学"),
        ("英语", "英语"),
        ("物理", "物理"),
        ("化学", "化学"),
        ("生物", "生物"),
        ("历史", "历史"),
        ("地理", "地理"),
        ("科学", "科学"),
        ("政治", "政治"),
        ("道德与法治", "政治"), // 映射由于"政治"
        ("道法", "政治"),       // 简写映射
        ("思品", "政治"),       // 思想品德
    ];

    let mut subject = "未知".to_string();
    for (keyword, standard_name) in &subject_mappings {
        if subject_text.contains(keyword) {
            subject = standard_name.to_string();
            break;
        }
    }

    // 如果从科目栏没找到，尝试从标题提取
    if subject == "未知" {
        warn!(
            "无法从科目栏识别科目(text: {})，尝试从标题提取...",
            subject_text
        );
        for (keyword, standard_name) in &subject_mappings {
            if title.contains(keyword) {
                subject = standard_name.to_string();
                info!("从标题中提取到科目: {}", subject);
                break;
            }
        }
    }

    if subject == "未知" {
        error!("❌ 无法识别科目！标题: {}, 科目栏: {}", title, subject_text);
        panic!(
            "无法识别科目，程序终止。标题: {}, 科目栏: {}",
            title, subject_text
        );
    } else {
        info!("识别到的科目: {}", subject);
    }

    let year = extract_year(&title).to_string();
    debug!("提取到的年份: {}", year);

    debug!("准备生成 PDF 文件");
    let pdf_dir = Path::new("PDF");
    if !pdf_dir.exists() {
        debug!("PDF 目录不存在，正在创建");
        fs::create_dir_all(pdf_dir).await?;
    }
    let name_for_pdf = sanitize_filename(&title);
    let pdf_path = pdf_dir.join(format!("{}.pdf", name_for_pdf));
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

    // 收集所有截图到 stem_screenshot_list
    let stem_screenshot_list: Vec<String> = questions
        .iter()
        .filter(|q| !q.is_title)
        .map(|q| q.screenshot.clone())
        .collect();

    Ok(Paper {
        name: title,
        province,
        grade,
        year,
        subject,
        page_id: None,
        stemlist: questions,
        stem_screenshot_list,
        name_for_pdf,
        url: paper_url.to_string(),
        is_exit: Some(false),
    })
}

/// 生成 PDF 文件
pub async fn generate_pdf(page: &chromiumoxide::Page, path: &Path) -> anyhow::Result<()> {
    let params = PrintToPdfParams::default();
    let _pdf_data = page.save_pdf(params, path).await?;
    Ok(())
}

/// 对每个题目进行截图并返回 base64 编码的列表
async fn capture_question_screenshots(
    state: &AppState,
    styles: &str,
    elements_data: &Value,
) -> anyhow::Result<Vec<String>> {
    let mut screenshots = Vec::new();

    let elements_array = elements_data["elements"]
        .as_array()
        .ok_or_else(|| anyhow!("无法获取 elements 数组"))?;

    // 创建唯一的临时目录（避免并发冲突）
    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "xueke_screenshots_{}_{}",
        std::process::id(),
        unique_id
    ));
    fs::create_dir_all(&temp_dir).await?;

    // 复用现有浏览器创建新页面
    debug!("复用全局浏览器创建截图页面");
    let page = state.browser.new_page("about:blank").await?;

    let mut question_index = 0;
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
                question_index += 1;
                let question_html = exam_item.html();

                // 构造完整的 HTML
                let html_content = format!(
                    r#"
                    <!DOCTYPE html>
                    <html lang="zh-CN">
                    <head>
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
                    </head>
                    <body>
                        <div class="exam-item__cnt">
                            {}
                        </div>
                    </body>
                    </html>
                    "#,
                    styles, question_html
                );

                // 保存为临时 HTML 文件
                let html_filename = format!("q_{}.html", question_index);
                let html_path = temp_dir.join(&html_filename);

                let mut file = fs::File::create(&html_path).await?;
                file.write_all(html_content.as_bytes()).await?;

                // 获取文件的 file:// URL
                let abs_path = std::fs::canonicalize(&html_path)?;
                let path_str = abs_path.to_string_lossy().to_string();

                #[cfg(windows)]
                let file_url = {
                    let clean_path = path_str.trim_start_matches(r"\\?\");
                    format!("file:///{}", clean_path.replace("\\", "/"))
                };

                #[cfg(not(windows))]
                let file_url = format!("file://{}", path_str);

                // 打开本地文件
                page.goto(&file_url).await?;

                // 等待图片加载
                tokio::time::sleep(Duration::from_millis(800)).await;

                // 截图
                let body_el = page.find_element("body").await?;
                let screenshot_bytes = body_el.screenshot(CaptureScreenshotFormat::Png).await?;

                // 转换为 base64
                let base64_str = general_purpose::STANDARD.encode(&screenshot_bytes);
                screenshots.push(base64_str);

                debug!("题目 {} 截图完成", question_index);
            }
        }
    }

    // 清理临时目录
    if let Err(e) = fs::remove_dir_all(&temp_dir).await {
        warn!("清理临时目录失败: {}", e);
    }

    // 关闭截图用的页面
    if let Err(e) = page.close().await {
        warn!("关闭截图页面失败: {}", e);
    }

    Ok(screenshots)
}
pub const ELEMENTS_DATA_JS: &str = r#"
        () => {
            const styles = Array.from(document.styleSheets)
                .map(sheet => {
                    try {
                        return Array.from(sheet.cssRules)
                            .map(rule => rule.cssText)
                            .join('\n');
                    } catch (e) {
                        return '';
                    }
                })
                .join('\n');
            const container = document.querySelector('.sec-item') ||
                            document.querySelector('.paper-content') ||
                            document.querySelector('body');
            if (!container) {
                return { styles: styles, elements: [] };
            }
            const allElements = Array.from(container.querySelectorAll('.sec-title, .sec-list'));
            const elements = [];
            allElements.forEach(el => {
                if (el.classList.contains('sec-title')) {
                    const span = el.querySelector('span');
                    const titleText = span ? span.innerText.trim() : '';
                    if (titleText) {
                        elements.push({
                            type: 'title',
                            title: titleText,
                            content: ''
                        });
                    }
                } else if (el.classList.contains('sec-list')) {
                    elements.push({
                        type: 'content',
                        title: '',
                        content: el.outerHTML
                    });
                }
            });
            return { styles: styles, elements: elements };
        }
    "#;

// 注入页面的 JS：提取 CSS 和处理过的 HTML
const EXTRACT_DATA_JS: &str = r#"
        () => {
            // 1. 提取所有 CSS
            // 能够处理内联样式和跨域 @import
            const styles = Array.from(document.styleSheets)
                .map(sheet => {
                    try {
                        // 尝试直接读取 CSS 规则
                        return Array.from(sheet.cssRules).map(rule => rule.cssText).join('\n');
                    } catch (e) {
                        // 如果跨域读取失败 (CORS)，则保留 import 链接
                        if (sheet.href) {
                            return `@import url("${sheet.href}");`;
                        }
                        return '';
                    }
                })
                .join('\n');

            // 2. 提取并清洗题目 HTML
            const questions = Array.from(document.querySelectorAll('.tk-quest-item'))
                .map(el => {
                    // 深拷贝一份，避免修改原页面显示
                    const clone = el.cloneNode(true);

                    // A. 移除底部的操作栏（加入试题篮、纠错等）
                    const ctrlBox = clone.querySelector('.ctrl-box');
                    if (ctrlBox) ctrlBox.remove();
                    
                    // B. 移除顶部的无关信息（例如"您最近一年使用..."）
                    const customInfo = clone.querySelector('.exam-item__custom');
                    if (customInfo) customInfo.remove();

                    // C. 【关键】处理图片懒加载
                    // 将 data-src 或 data-original 强制赋值给 src
                    clone.querySelectorAll('img').forEach(img => {
                        const realSrc = img.getAttribute('data-src') || img.getAttribute('data-original');
                        if (realSrc) {
                            img.src = realSrc;
                        }
                        // 确保公式图片垂直居中
                        img.style.verticalAlign = 'middle';
                    });

                    return clone.outerHTML;
                });

            return { styles, questions };
        }
    "#;

const TITLE_JS: &str = r#"
        () => {
            const titleElement = document.querySelector('.title-txt .txt');
            return titleElement ? titleElement.innerText : '未找到标题';
        }
    "#;

const INFO_JS: &str = r#"
        () => {
            const items = document.querySelectorAll('.info-list .item');
            if (items.length >= 2) {
                return {
                    shengfen: items[0].innerText.trim(),
                    nianji: items[1].innerText.trim()
                };
            }
            return { shengfen: '未找到', nianji: '未找到' };
        }
    "#;

const SUBJECT_JS: &str = r#"
        () => {
            const subjectElement = document.querySelector('.subject');
            return subjectElement ? subjectElement.innerText : '未找到科目';
        }
    "#;

/// 清理文件名中的非法字符
fn sanitize_filename(filename: &str) -> String {
    filename
        .replace("/", "_")
        .replace("\\", "_")
        .replace(":", "_")
        .replace("*", "_")
        .replace("?", "_")
        .replace("\"", "_")
        .replace("<", "_")
        .replace(">", "_")
        .replace("|", "_")
}

/// 从标题中提取年份
fn extract_year(title: &str) -> &str {
    // 尝试匹配 4 位数字年份
    if let Some(start) = title.find(|c: char| c.is_ascii_digit()) {
        let year_str = &title[start..];
        if year_str.len() >= 4 {
            let potential_year = &year_str[..4];
            if potential_year.chars().all(|c| c.is_ascii_digit()) {
                let year_num: u32 = potential_year.parse().unwrap_or(0);
                if year_num >= 2000 && year_num <= 2100 {
                    return potential_year;
                }
            }
        }
    }
    "未知"
}
