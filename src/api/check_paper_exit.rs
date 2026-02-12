use anyhow::Result;
use tracing::info;

use super::get_request::send_api_get_request;

/// 检查试卷名称是否重复
///
/// # 参数
/// * `paper_name` - 试卷名称
/// * `operation_type` - 操作类型 (1: 新建, 2: 编辑)
/// * `paper_id` - 试卷ID (新建时为空)
///
/// # 返回
/// * `Ok(bool)` - true表示重复，false表示不重复
pub async fn check_paper_name_exist(
    paper_name: &str,
    paper_id: Option<&str>,
) -> Result<bool> {
    let paper_id_str = paper_id.unwrap_or("");
    let operation_type = 1;

    // URL 编码试卷名称
    let encoded_paper_name = urlencoding::encode(paper_name);

    let url = format!(
        "https://tps-tiku-api.staff.xdf.cn/paper/check/paperName?paperName={}&operationType={}&paperId={}",
        encoded_paper_name, operation_type, paper_id_str
    );

    let resp = send_api_get_request(&url).await?;

    let is_repeated = resp
        .get("data")
        .and_then(|data| data.get("repeated"))
        .and_then(|v| v.as_bool())
        .unwrap();

    if is_repeated {
        info!("试卷名称 '{}' 已存在", paper_name);
    } else {
        info!("试卷名称 '{}' 可用", paper_name);
    }

    Ok(is_repeated)
}

#[cfg(test)]
mod tests {
    use crate::app::logger;

    use super::*;

    #[tokio::test]
    async fn test_check_paper_name_exist() {
        logger::init_test();

        // 测试新建试卷时检查名称
        let result = check_paper_name_exist("山东省临沂市临沭县东城实验中学2024-2025学年八年级上学期10月月考地理试题",  None).await;
        assert_eq!(result.is_ok(), true);
        // println!("试卷名称检查结果: {:?}", result);
    }
}
