/// 清理文件名中的非法字符
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .replace("/", "_")
        .replace("\\", "_")
        .replace(":", "_")
        .replace("*", "_")
        .replace("?", "_")
        .replace("\"", "_")
        .replace("<", "_")
        .replace(">", "_")
        .replace("|", "_")
}

/// 从标题中提取年份
pub fn extract_year(title: &str) -> &str {
    // 尝试匹配 4 位数字年份
    if let Some(start) = title.find(|c: char| c.is_ascii_digit()) {
        let year_str = &title[start..];
        if year_str.len() >= 4 {
            let potential_year = &year_str[..4];
            if potential_year.chars().all(|c| c.is_ascii_digit()) {
                let year_num: u32 = potential_year.parse().unwrap_or(0);
                if year_num >= 2000 && year_num <= 2100 {
                    return potential_year;
                }
            }
        }
    }
    "未知"
}
