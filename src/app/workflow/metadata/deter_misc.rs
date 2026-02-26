use tokio_retry::strategy::FixedInterval;
use tracing::warn;

use crate::api::llm::ask_llm;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MiscInfo {
    pub paper_type_name: String,
    pub parent_paper_type: String,
    pub school_year_begin: Option<i32>,
    pub school_year_end: Option<i32>,
    pub paper_term: Option<String>,
    pub paper_year: Option<i32>,
    pub paper_month: Option<i32>,
}

impl Default for MiscInfo {
    fn default() -> Self {
        Self {
            paper_type_name: String::new(),
            parent_paper_type: String::new(),
            school_year_begin: Some(2024),
            school_year_end: Some(2025),
            paper_term: Some("1".to_string()),
            paper_month: None,
            paper_year: None,
        }
    }
}

impl MiscInfo {
    pub fn misc_prompt_for_llm(question_name: &str) -> String {
        let user_message = format!(
            r#"你是一个专业的教务数据分析助手。请根据试卷名称 "{}" 分析并提取元数据。

请严格遵守以下规则，返回一个纯 JSON 对象，不要包含 markdown 格式标记（如 ```json ... ```）。只用给我返回Json 对象就可以了！！！
请严格遵守以下规则，返回一个纯 JSON 对象，不要包含 markdown 格式标记（如 ```json ... ```）。只用给我返回Json 对象就可以了！！！
请严格遵守以下规则，返回一个纯 JSON 对象，不要包含 markdown 格式标记（如 ```json ... ```）。只用给我返回Json 对象就可以了！！！

### 字段定义与约束：

1. **paper_type_name** (String): 试卷类型。必须从以下列表中选择最合适的一个：
   - 中考真题, 中考模拟, 学业考试, 自主招生
   - 小初衔接, 初高衔接
   - 期中考试, 期末考试, 单元测试, 开学考试, 月考, 周测, 课堂闭环, 阶段测试
   - 教材, 教辅
   - 竞赛
   【⚠️重要特殊规则】：所有非常规卷名（如带有【智】等特殊前缀的、以“专题n”开头的、没有明显常规考试字眼的卷名），一律归类为 "教辅"。

2. **parent_paper_type** (String): 试卷大类。请根据你选择的 `paper_type_name`，按照以下映射关系自动填充：
   - 若类型为[中考真题, 中考模拟, 学业考试, 自主招生] -> 归类为 "中考专题"
   - 若类型为[小初衔接, 初高衔接] -> 归类为 "跨学段衔接"
   - 若类型为[期中考试, 期末考试, 单元测试, 开学考试, 月考, 周测, 课堂闭环, 阶段测试] -> 归类为 "阶段测试"
   - 若类型为 [教材, 教辅] -> 归类为 "新东方自研"
   - 若类型为 [竞赛] -> 归类为 "竞赛"

3. **school_year_begin** (i32): 学年开始年份。例如 2023。
4. **school_year_end** (i32): 学年结束年份。例如 2024。
   - 逻辑参考：
     - 2023-2024 -> begin=2023, end=2024
     - 2024年下学期(春季) -> 属于 2023-2024 学年 -> begin=2023, end=2024
     - 2024年上学期(秋季) -> 属于 2024-2025 学年 -> begin=2024, end=2025

5. **paper_year** (i32): 试卷具体年份。根据学年和学期严格按以下逻辑推算：
   - 所有下学期（春季）的试卷，年份取学年结束年份（后一年）。
   - 所有上学期（秋季）的“期末考试”，年份取学年结束年份（后一年）。
   - 上学期的其他考试（如期中、月考等），年份取学年开始年份（前一年）。
   - 示例1：2024-2025上学期期末 -> paper_year=2025
   - 示例2：2024-2025下学期月考 -> paper_year=2025
   - 示例3：2024-2025上学期期中 -> paper_year=2024

6. **paper_term** (String): 学期。**注意：必须返回字符串类型的数字**。
   - "1" 代表上学期（秋季）
   - "2" 代表下学期（春季）
   - 如果标题中没有这个信息，返回 null

7. **paper_month** (Integer): 考试月份。**注意：必须返回整数**。
   - 如果标题中没有这个信息，返回 null

### JSON 返回示例：
{{
  "paper_type_name": "教辅",
  "parent_paper_type": "新东方自研",
  "school_year_begin": 2024,
  "school_year_end": 2025,
  "paper_year": 2025,
  "paper_term": "1", 
  "paper_month": null
}}
"#,
            question_name
        );
        user_message
    }

    pub async fn get_misc_info(question_name: &str) -> Option<MiscInfo> {
        // 1. 定义策略：重试4次，每次间隔1000ms
        let strategy = FixedInterval::from_millis(1000).take(4);

        let action = || async {
            let prompt = Self::misc_prompt_for_llm(question_name);
            let response = ask_llm(&prompt).await.map_err(|_| "LLM 请求失败")?;

            let cleaned = response
                .trim()
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```");

            serde_json::from_str::<MiscInfo>(cleaned).map_err(|e| {
                warn!("解析失败: {}, 原始内容: {}", e, response);
                "JSON 解析失败"
            })
        };

        // 3. 执行：一行代码搞定重试循环
        match tokio_retry::Retry::spawn(strategy, action).await {
            Ok(info) => Some(info),
            Err(e) => {
                warn!("重试耗尽，最终失败原因: {:?}", e);
                None
            }
        }
    }
}
