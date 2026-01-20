/// JavaScript 脚本：提取页面元素数据（样式和题目）
pub const ELEMENTS_DATA_JS: &str = r#"
        () => {
            const styles = Array.from(document.styleSheets)
                .map(sheet => {
                    try {
                        return Array.from(sheet.cssRules)
                            .map(rule => rule.cssText)
                            .join('\n');
                    } catch (e) {
                        return '';
                    }
                })
                .join('\n');
            const container = document.querySelector('.sec-item') ||
                            document.querySelector('.paper-content') ||
                            document.querySelector('body');
            if (!container) {
                return { styles: styles, elements: [] };
            }
            const allElements = Array.from(container.querySelectorAll('.sec-title, .sec-list'));
            const elements = [];
            allElements.forEach(el => {
                if (el.classList.contains('sec-title')) {
                    const span = el.querySelector('span');
                    const titleText = span ? span.innerText.trim() : '';
                    if (titleText) {
                        elements.push({
                            type: 'title',
                            title: titleText,
                            content: ''
                        });
                    }
                } else if (el.classList.contains('sec-list')) {
                    elements.push({
                        type: 'content',
                        title: '',
                        content: el.outerHTML
                    });
                }
            });
            return { styles: styles, elements: elements };
        }
    "#;

/// JavaScript 脚本：提取 CSS 和处理过的 HTML（备用）
#[allow(dead_code)]
pub const EXTRACT_DATA_JS: &str = r#"
        () => {
            // 1. 提取所有 CSS
            // 能够处理内联样式和跨域 @import
            const styles = Array.from(document.styleSheets)
                .map(sheet => {
                    try {
                        // 尝试直接读取 CSS 规则
                        return Array.from(sheet.cssRules).map(rule => rule.cssText).join('\n');
                    } catch (e) {
                        // 如果跨域读取失败 (CORS)，则保留 import 链接
                        if (sheet.href) {
                            return `@import url("${sheet.href}");`;
                        }
                        return '';
                    }
                })
                .join('\n');

            // 2. 提取并清洗题目 HTML
            const questions = Array.from(document.querySelectorAll('.tk-quest-item'))
                .map(el => {
                    // 深拷贝一份，避免修改原页面显示
                    const clone = el.cloneNode(true);

                    // A. 移除底部的操作栏（加入试题篮、纠错等）
                    const ctrlBox = clone.querySelector('.ctrl-box');
                    if (ctrlBox) ctrlBox.remove();
                    
                    // B. 移除顶部的无关信息（例如"您最近一年使用..."）
                    const customInfo = clone.querySelector('.exam-item__custom');
                    if (customInfo) customInfo.remove();

                    // C. 【关键】处理图片懒加载
                    // 将 data-src 或 data-original 强制赋值给 src
                    clone.querySelectorAll('img').forEach(img => {
                        const realSrc = img.getAttribute('data-src') || img.getAttribute('data-original');
                        if (realSrc) {
                            img.src = realSrc;
                        }
                        // 确保公式图片垂直居中
                        img.style.verticalAlign = 'middle';
                    });

                    return clone.outerHTML;
                });

            return { styles, questions };
        }
    "#;

/// JavaScript 脚本：提取试卷标题
pub const TITLE_JS: &str = r#"
        () => {
            const titleElement = document.querySelector('.title-txt .txt');
            return titleElement ? titleElement.innerText : '未找到标题';
        }
    "#;

/// JavaScript 脚本：提取省份和年级信息
pub const INFO_JS: &str = r#"
        () => {
            const items = document.querySelectorAll('.info-list .item');
            if (items.length >= 2) {
                return {
                    shengfen: items[0].innerText.trim(),
                    nianji: items[1].innerText.trim()
                };
            }
            return { shengfen: '未找到', nianji: '未找到' };
        }
    "#;

/// JavaScript 脚本：提取科目信息
pub const SUBJECT_JS: &str = r#"
        () => {
            const menuTitle = document.querySelector('.subject-menu__title .title-txt');
            if (menuTitle) {
                return menuTitle.innerText.trim();
            }

            const subjectElement = document.querySelector('.subject');
            return subjectElement ? subjectElement.innerText.trim() : '未找到科目';
        }
    "#;
