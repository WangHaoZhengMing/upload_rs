use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverterFile {
    pub attachment_id: String,
    pub file_name: String,
    pub file_url: String,
    pub path: String,
    pub file_type: String,
}

/// 附件响应信息
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentData {
    pub attachment_id: String,
    pub file_name: String,
    pub file_url: String,
    pub path: String,
    pub file_type: String,
    pub converter_files: Vec<ConverterFile>,
}

/// 批量上传响应
#[derive(Debug, Deserialize)]
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

/// 完整流程：上传 PDF -> 批量上传 -> 获取转换后的图片 URLs
pub async fn upload_and_convert_pdf(local_pdf_path: &str) -> Result<BatchUploadResponse> {
    // 1. 上传 PDF 到腾讯云 COS
    let pdf_url = upload_pdf(local_pdf_path).await?;

    // 2. 提取文件名
    let file_name = std::path::Path::new(local_pdf_path)
        .file_name()
        .and_then(|n| n.to_str())
        .context("无法获取文件名")?;

    // 3. 调用批量上传接口
    let response_json = batch_upload_files(&pdf_url, file_name).await?;

    // 4. 解析响应
    let response: BatchUploadResponse =
        serde_json::from_value(response_json).context("解析批量上传响应失败")?;

    if !response.success {
        return Err(anyhow::anyhow!("批量上传失败: {}", response.message));
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
