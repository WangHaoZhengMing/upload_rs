use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;
use tokio::io::AsyncWriteExt;

use super::pdf::upload_pdf;
use crate::api::send_request::send_api_request;

/// 上传附件信息
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadAttachment {
    file_name: String,
    file_type: String,
    file_url: String,
    resource_type: String,
}

/// 批量上传请求
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchUploadRequest {
    upload_attachments: Vec<UploadAttachment>,
}

/// 转换后的文件信息
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverterFile {
    pub attachment_id: String,
    pub file_name: String,
    pub file_url: String,
    pub path: String,
    pub file_type: String,
}

/// 附件响应信息
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentData {
    pub attachment_id: String,
    pub file_name: String,
    pub file_url: String,
    pub path: String,
    pub file_type: String,

    // 加上 default 属性，防止字段缺失导致解析报错
    #[serde(default)]
    pub converter_files: Vec<ConverterFile>,
}

/// 批量上传响应
#[derive(Debug, Deserialize, Serialize)]
pub struct BatchUploadResponse {
    pub success: bool,
    pub code: u16,
    pub message: String,
    pub data: Vec<AttachmentData>,
}

/// 调用批量上传接口，将 PDF URL 提交到服务器进行转换
async fn batch_upload_files(pdf_url: &str, file_name: &str) -> Result<Value> {
    let attachment = UploadAttachment {
        file_name: file_name.to_string(),
        file_type: "application/pdf".to_string(),
        file_url: pdf_url.to_string(),
        resource_type: "zbtiku_pc".to_string(),
    };

    let request = BatchUploadRequest {
        upload_attachments: vec![attachment],
    };

    let payload = serde_json::to_value(&request)?;

    info!("提交批量上传请求，PDF URL: {}", pdf_url);

    send_api_request(
        "https://tps-tiku-api.staff.xdf.cn/attachment/batch/upload/files",
        &payload,
    )
    .await
}

/// 写入警告信息到文件
async fn write_warning_to_file(message: &str) -> Result<()> {
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("warn.txt")
        .await?;
    
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_line = format!("[{}] {}\n", timestamp, message);
    file.write_all(log_line.as_bytes()).await?;
    
    Ok(())
}

/// 完整流程：上传 PDF -> 批量上传 -> 获取转换后的图片 URLs
pub async fn upload_and_convert_pdf(local_pdf_path: &str) -> Result<BatchUploadResponse> {
    // 1. 上传 PDF 到腾讯云 COS
    let pdf_url = upload_pdf(local_pdf_path).await?;

    // 2. 提取文件名
    let file_name = std::path::Path::new(local_pdf_path)
        .file_name()
        .and_then(|n| n.to_str())
        .context("无法获取文件名")?;

    // 3. 调用批量上传接口 (带有重试机制)
    let max_retries = 3;
    let mut response_json = serde_json::Value::Null;

    for i in 1..=max_retries {
        // 调用批量上传接口
        response_json = batch_upload_files(&pdf_url, file_name).await?;

        // 简单检查 JSON 结构是否合法，如果不合法直接下一次重试
        // 注意：这里只是初步拿到 JSON，具体数据是否为空要在解析后判断

        let should_retry_empty_data =
            if let Some(data) = response_json.get("data").and_then(|d| d.as_array()) {
                if data.is_empty() {
                    true
                } else {
                    // 进一步检查 converterFiles 是否为空
                    // 只要有一个 attachment 的 converterFiles 不为空，我们就算成功
                    let has_converted_files = data.iter().any(|item| {
                        item.get("converterFiles")
                            .and_then(|cf| cf.as_array())
                            .map(|arr| !arr.is_empty())
                            .unwrap_or(false)
                    });
                    !has_converted_files
                }
            } else {
                true // data 字段不存在或不是数组，视为 data 为空
            };

        if !should_retry_empty_data {
            if i > 1 {
                info!("第 {} 次尝试获取到有效数据", i);
            }
            break;
        }

        if i < max_retries {
            let warn_msg = format!("批量上传返回 data 为空，准备进行第 {} 次重试... PDF: {}", i + 1, file_name);
            tracing::warn!("{}", warn_msg);
            
            // 写入警告文件
            if let Err(e) = write_warning_to_file(&warn_msg).await {
                tracing::error!("写入警告文件失败: {}", e);
            }
            
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        } else {
            let error_msg = format!("重试 {} 次后 data 依然为空，PDF: {}", max_retries, file_name);
            tracing::error!("{}", error_msg);
            
            // 写入警告文件
            if let Err(e) = write_warning_to_file(&error_msg).await {
                tracing::error!("写入警告文件失败: {}", e);
            }
        }
    }

    // 4. 解析响应
    let response: BatchUploadResponse =
        serde_json::from_value(response_json).context("解析批量上传响应失败")?;

    if !response.success {
        return Err(anyhow::anyhow!("批量上传失败: {}", response.message));
    }

    // 再次硬性检查 data 是否为空，虽然前面循环已经尽力了
    if response.data.is_empty() {
        return Err(anyhow::anyhow!(
            "批量上传成功但 converter_files 为空 (已重试 {} 次)",
            max_retries
        ));
    }

    info!("PDF 转换成功，共 {} 个附件", response.data.len());
    if let Some(first) = response.data.first() {
        info!("转换后的图片数量: {}", first.converter_files.len());
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::logger;

    #[tokio::test]
    async fn test_upload_and_convert_pdf() {
        logger::init_test();

        let file_path = "chat-aoax6xxa5t.pdf";

        if std::path::Path::new(file_path).exists() {
            println!("找到测试文件，开始上传和转换: {}", file_path);

            let result = upload_and_convert_pdf(file_path).await;

            match result {
                Ok(response) => {
                    println!("上传并转换成功！");
                    println!("响应: {:?}", response);

                    if let Some(attachment) = response.data.first() {
                        println!("PDF 附件 ID: {}", attachment.attachment_id);
                        println!("转换后的图片:");
                        for (i, img) in attachment.converter_files.iter().enumerate() {
                            println!("  [{}] {}: {}", i, img.file_name, img.file_url);
                        }
                    }
                }
                Err(e) => {
                    println!("上传出错: {:?}", e);
                    panic!("测试失败");
                }
            }
        } else {
            println!("未找到测试文件 {}", file_path);
        }
    }
}
