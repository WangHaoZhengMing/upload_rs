use base64::{Engine as _, engine::general_purpose};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

/// 将单个题目转换为图片
///
/// # 参数
/// * `head_html` - HTML head 内容
/// * `question_html` - 题目的 HTML 内容
/// * `index` - 题目索引（用于生成唯一文件名）
///
/// # 返回
/// 返回生成的图片的 base64 编码
pub fn render_question_to_image(
    head_html: &str,
    question_html: &str,
    index: usize,
) -> anyhow::Result<String> {
    // 检查环境
    if Command::new("wkhtmltoimage")
        .arg("--version")
        .output()
        .is_err()
    {
        anyhow::bail!("未找到 'wkhtmltoimage' 命令");
    }

    // 获取系统临时目录
    let temp_dir = env::temp_dir();
    let process_id = std::process::id();
    let thread_id = std::thread::current().id();

    // 生成唯一的文件名（避免多线程冲突）
    let unique_prefix = format!("question_{}_{:?}_{}", process_id, thread_id, index);

    // 构造 HTML
    let single_question_page = format!(
        r#"
        <!DOCTYPE html>
        <html lang="zh-cn">
        <head>
            {head_content}
            <style>
                body {{
                    background-color: #fff;
                    padding: 20px;
                    font-family: "Microsoft YaHei", "微软雅黑", sans-serif !important;
                }}
                .tk-quest-item, table, td, tr, div, span, p, a {{
                    font-family: "Microsoft YaHei", "微软雅黑", sans-serif !important;
                }}
                .tk-quest-item {{ margin: 0 auto; width: 100%; }}
            </style>
        </head>
        <body>
            {question_content}
        </body>
        </html>
        "#,
        head_content = head_html,
        question_content = question_html
    );

    // 在临时目录中创建文件
    let temp_filename = temp_dir.join(format!("{}.html", unique_prefix));
    let output_filename = temp_dir.join(format!("{}.jpg", unique_prefix));

    // 写入临时文件
    let mut file = File::create(&temp_filename)?;
    file.write_all(single_question_page.as_bytes())?;

    // 调用 wkhtmltoimage
    let status = Command::new("wkhtmltoimage")
        .arg("--quality")
        .arg("95")
        .arg("--width")
        .arg("1000")
        .arg("--disable-smart-width")
        .arg("--enable-local-file-access")
        .arg("--load-error-handling")
        .arg("ignore")
        .arg("--disable-javascript")
        .arg(&temp_filename)
        .arg(&output_filename)
        .output()?;

    if !status.status.success() {
        anyhow::bail!("wkhtmltoimage 执行失败");
    }

    // 读取图片文件并转换为 base64
    let image_data = fs::read(&output_filename)?;
    let base64_string = general_purpose::STANDARD.encode(&image_data);

    // 删除临时文件（操作系统会定期清理临时目录，但手动删除更好）
    let _ = fs::remove_file(temp_filename);
    let _ = fs::remove_file(output_filename);

    Ok(base64_string)
}
