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

/// 从标题中提取年份 (范围 2010-2040)
pub fn extract_year(title: &str) -> String {
    // 将字符串转为字符向量，这样处理中文绝对安全，下标是按字符算的
    let chars: Vec<char> = title.chars().collect();

    // 遍历字符数组，查找连续的4个字符
    // saturating_sub 防止字符串短于4个字符时溢出
    for i in 0..chars.len().saturating_sub(3) {
        // 预先检查：只有当这4个全是数字时才转换，提高性能
        if chars[i].is_ascii_digit()
            && chars[i + 1].is_ascii_digit()
            && chars[i + 2].is_ascii_digit()
            && chars[i + 3].is_ascii_digit()
        {
            // 收集这4个字符组成字符串
            let year_str: String = chars[i..i + 4].iter().collect();

            // 转换数字并判断范围
            if let Ok(year_num) = year_str.parse::<u32>() {
                if year_num >= 2010 && year_num <= 2040 {
                    return year_str;
                }
            }
        }
    }

    "2026".to_string()
}
