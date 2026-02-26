use anyhow::{Ok, Result};
use serde_json::{Value, json};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, warn};

use crate::api::upload::batch::upload_and_convert_pdf;
use crate::app::models::Paper;
use crate::app::workflow::metadata::data_addr::get_province_code;
use crate::app::workflow::metadata::data_grade::find_grade_code;
use crate::app::workflow::metadata::data_paper_type::{PaperCategory, get_subtype_value_by_name};
use crate::app::workflow::metadata::data_subject::find_subject_code;
use crate::app::workflow::metadata::deter_city::determine_city_from_paper_name;
use crate::app::workflow::metadata::deter_misc::MiscInfo;

pub async fn save_paper(paper: &mut Paper) -> anyhow::Result<()> {
    let playload = construct_upload_payload(paper).await?;
    debug!("上传试卷负载: {}", playload);

    // 调用 API 提交试卷
    let json_val = crate::api::submit_paper::submit_paper_api(&playload).await?;

    debug!("保存试卷响应: {:?}", json_val);

    // 3. 提取 data 字段
    if let Some(data_str) = json_val.get("data").and_then(|v| v.as_str()) {
        let paper_id = data_str.to_string();
        info!("Paper ID: {}", paper_id);
        paper.set_paper_id(paper_id);
    } else {
        error!("无法找到保存试卷的 data 字段或 data 不是字符串");
        debug!("失败的完整响应: {:?}", json_val);
    }

    // 导出 paper 到 TOML 文件
    export_paper_to_toml(paper).await?;

    Ok(())
}

async fn construct_upload_payload(paper: &Paper) -> Result<Value> {
    let city_code = determine_city_from_paper_name(&paper.name, &paper.province)
        .await?
        .unwrap_or(0)
        .to_string();
    let province_code = get_province_code(&paper.province).unwrap_or(0);
    let parsed_data = MiscInfo::get_misc_info(&paper.name).await.unwrap();

    let pdf_path = format!("PDF/{}.pdf", paper.name_for_pdf);
    let upload_response = upload_and_convert_pdf(&pdf_path).await?;
    let attachments = serde_json::to_value(&upload_response.data)?;

    let payload = json!({
        "paperType":get_subtype_value_by_name(&paper.subject,&parsed_data.paper_type_name),

        "parentPaperType": PaperCategory::get_value(&parsed_data.parent_paper_type).unwrap_or_else(||{warn!("Not found parentPaperType, using default"); "ppt1"}),
        "schName": "集团",
        "schNumber": "65",
        "paperMonth": parsed_data.paper_month,
        "schoolYearBegin": parsed_data.school_year_begin.unwrap_or_else(||2025),
        "schoolYearEnd": parsed_data.school_year_end.unwrap_or_else(||2026),
        "paperTerm": parsed_data.paper_term.unwrap_or_else(||{warn!("not found paper_term, using \"1\" by default");"1".to_string()}),
        "paperYear": parsed_data.paper_year.or_else(||{warn!("not found paper_year, using 2025 by default"); Some(2025)}),
        "courseVersionCode": "",
        "address": [
        {
            "province": province_code.to_string(),
            "city": city_code
        }
        ],
        "title": &paper.name,
        "stage": "3",
        "stageName": "初中",
        "subject": find_subject_code(&paper.subject).unwrap().to_string(),
        "subjectName": &paper.subject,
        "gradeName": &paper.grade,
        "grade": find_grade_code(&paper.grade).unwrap_or_else(||{warn!("Can not infer grade or find. Using 161 default"); 161}).to_string(),

        "paperId": "",
        "attachments": attachments
    });

    Ok(payload)
}

/// 导出 Paper 到 TOML 文件
async fn export_paper_to_toml(paper: &Paper) -> Result<()> {
    // 创建 output_toml 目录
    let output_dir = "output_toml";
    if !std::path::Path::new(output_dir).exists() {
        fs::create_dir_all(output_dir).await?;
        debug!("创建目录: {}", output_dir);
    }

    // 序列化为 TOML
    let toml_string = toml::to_string_pretty(paper)?;

    // 清理文件名中的非法字符
    let safe_filename = paper
        .name
        .replace("/", "_")
        .replace("\\", "_")
        .replace(":", "_")
        .replace("*", "_")
        .replace("?", "_")
        .replace("\"", "_")
        .replace("<", "_")
        .replace(">", "_")
        .replace("|", "_");

    // 写入文件
    let file_path = format!("{}/{}.toml", output_dir, safe_filename);
    let mut file = fs::File::create(&file_path).await?;
    file.write_all(toml_string.as_bytes()).await?;

    info!("导出 Paper 到: {}", file_path);
    Ok(())
}
