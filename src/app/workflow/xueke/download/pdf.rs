use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use std::path::Path;

/// 生成 PDF 文件
pub async fn generate_pdf(page: &chromiumoxide::Page, path: &Path) -> anyhow::Result<()> {
    let params = PrintToPdfParams::default();
    let _pdf_data = page.save_pdf(params, path).await?;
    Ok(())
}
