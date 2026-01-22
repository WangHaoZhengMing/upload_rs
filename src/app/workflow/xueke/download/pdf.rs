use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use std::path::Path;
use tracing::warn;

/// 生成 PDF 文件
pub async fn generate_pdf(page: &chromiumoxide::Page, path: &Path) -> anyhow::Result<()> {
    let mut attempts = 0;
    loop {
        attempts += 1;
        let params = PrintToPdfParams::default();
        match page.save_pdf(params, path).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                if attempts >= 3 {
                    return Err(e.into());
                }
                warn!("PDF generation failed (attempt {}): {}, retrying...", attempts, e);
            }
        }
    }
}
