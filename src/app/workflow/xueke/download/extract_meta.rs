use serde_json::Value;
use tracing::{debug, error, info, warn};

use crate::api::llm::{ask_llm_with_config, LlmConfig};
use super::scripts::{INFO_JS, SUBJECT_JS, TITLE_JS};
use super::utils::{extract_year, sanitize_filename};

/// 试卷元数据
pub struct PaperMetadata {
    pub title: String,
    pub name_for_pdf: String,
    pub province: String,
    pub grade: String,
    pub subject: String,
    pub year: String,
}

/// 从页面提取试卷元数据
pub async fn extract_paper_metadata(
    page: &chromiumoxide::Page,
) -> anyhow::Result<PaperMetadata> {
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
    let mut subject_text_clean = subject_text.clone();
    for noise in ["初中", "高中", "小学", "中考", "高考"] {
        subject_text_clean = subject_text_clean.replace(noise, "");
    }
    subject_text_clean = subject_text_clean.trim().to_string();
    debug!("提取到的科目文本: {} -> {}", subject_text, subject_text_clean);

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
        if subject_text_clean.contains(keyword) {
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

    // 如果还是未知，使用 AI 识别
    if subject == "未知" {
        warn!("从标题也无法识别科目，尝试使用 AI 识别...");
        
        let prompt = format!(
            "请从以下试卷标题中识别科目，只返回科目名称，不要包含其他内容。不要包含其他内容。不要包含其他内容。不要包含其他内容。不要包含其他内容。\n\
            可选的科目有：语文、数学、英语、物理、化学、生物、历史、地理、科学、道德与法治\n\
            如果无法识别，请返回\"未知\"\n\n\
            试卷标题：{}", 
            title
        );
        
        let config = LlmConfig {
            system_message: Some(
                "你是一个专业的科目识别助手，能够从试卷标题中准确识别科目。".to_string()
            ),
            ..Default::default()
        };
        
        match ask_llm_with_config(&prompt, config).await {
            Ok(ai_subject) => {
                let ai_subject = ai_subject.trim();
                // 验证 AI 返回的科目是否有效
                let valid_subjects = ["语文", "数学", "英语", "物理", "化学", "生物", "历史", "地理", "科学", "道德与法治"];
                if valid_subjects.contains(&ai_subject) {
                    subject = ai_subject.to_string();
                    info!("AI 识别到的科目: {}", subject);
                } else {
                    warn!("AI 返回的科目 '{}' 无效", ai_subject);
                }
            }
            Err(e) => {
                warn!("AI 识别科目失败: {}", e);
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

    let name_for_pdf = sanitize_filename(&title);

    Ok(PaperMetadata {
        title,
        name_for_pdf,
        province,
        grade,
        subject,
        year,
    })
}
