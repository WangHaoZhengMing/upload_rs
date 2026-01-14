use anyhow::{Ok, Result};
use chromiumoxide::Page;
use serde_json::{Value, json};
use tracing::warn;
use tracing::{debug, error, info};

use crate::api::upload::batch::upload_and_convert_pdf;
use crate::app::models::Paper;
use crate::app::workflow::metadata::data_addr::get_province_code;
use crate::app::workflow::metadata::data_grade::find_grade_code;
use crate::app::workflow::metadata::data_paper_type::{PaperCategory, get_subtype_value_by_name};
use crate::app::workflow::metadata::data_subject::find_subject_code;
use crate::app::workflow::metadata::deter_city::determine_city_from_paper_name;
use crate::app::workflow::metadata::deter_misc::MiscInfo;

pub async fn save_paper(tiku_page: Page, paper: &mut Paper) -> anyhow::Result<()> {
    let playload = construct_upload_payload(paper).await?;
    debug!("上传试卷负载: {}", playload);
    let code = build_save_paper_js(&playload);
    debug!("保存试卷的 JS 代码: {}", code);
    let response: chromiumoxide::js::EvaluationResult = tiku_page.evaluate(code).await?;
    debug!("保存试卷响应: {:?}", response);
    // 2. 转为通用的 JSON Value
    let json_val: Value = response.into_value()?;

    // 3. 提取 data 字段
    // 注意：as_str() 返回 Option<&str>，需要处理 None 的情况
    if let Some(data_str) = json_val.get("data").and_then(|v| v.as_str()) {
        let paper_id = data_str.to_string();
        info!("Paper ID: {}", paper_id);
        paper.set_paper_id(paper_id);
    } else {
        error!("无法找到 data 字段或 data 不是字符串");
    }
    
    Ok(())
}

async fn construct_upload_payload(paper: &Paper) -> Result<String> {
    let city_code = determine_city_from_paper_name(&paper.name, &paper.province)
        .await?
        .unwrap_or(0)
        .to_string();
    let province_code = get_province_code(&paper.province).unwrap_or(0);
    let parsed_data = MiscInfo::get_mis_info(&paper.name).await.unwrap();

    let pdf_path = format!("PDF/{}.pdf", paper.name_for_pdf);
    let upload_response = upload_and_convert_pdf(&pdf_path).await?;
    let attachments = serde_json::to_value(&upload_response.data)?;

    let payload = json!({
        "paperType":get_subtype_value_by_name(&paper.subject,&parsed_data.paper_type_name),

        "parentPaperType": PaperCategory::get_value(&parsed_data.parent_paper_type).unwrap_or_else(||{warn!("Not found parentPaperType, using default"); "ppt1"}),
        "schName": "集团",
        "schNumber": "65",
        "paperMonth": parsed_data.paper_month,
        "schoolYearBegin": parsed_data.school_year_begin,
        "schoolYearEnd": parsed_data.school_year_end,
        "paperTerm": parsed_data.paper_term.unwrap_or_else(||{warn!("not found paper_term, using \"\" by default");"".to_string()}),
        "paperYear": paper.year.parse::<i32>().unwrap_or_else(|_|{warn!("Can not parse year, using 2024 by default"); 2024}),
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

    Ok(payload.to_string())
}


const API_BASE_URL: &str = "https://tps-tiku-api.staff.xdf.cn";
const SAVE_PAPER_API_PATH: &str = "/paper/new/save";
/// 生成保存试卷的 JavaScript 代码
pub fn build_save_paper_js(playload: &str) -> String {
    format!(
        r#"
        (async () => {{
            try {{
                const response = await fetch("{API_BASE_URL}{SAVE_PAPER_API_PATH}", {{
                    method: "POST",
                    headers: {{
                        "Content-Type": "application/json",
                        "Accept": "application/json, text/plain, */*",
                        "tikutoken": "732FD8402F95087CD934374135C46EE5"
                    }},
                    credentials: "include",
                    body: JSON.stringify({}),
                }});
                const data = await response.json();
                return data;
            }} catch (err) {{
                return {{ error: err.toString() }};
            }}
        }})()
        "#,
        playload
    )
}
