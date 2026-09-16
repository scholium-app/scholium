//! LaTeX 宿主/组件驱动：真实调用 `xelatex`，读回 `.aux`、逐页文本与链接注解。
//!
//! 不猜测、不复用上一次结果：每轮都重新编译并重新解析产物。

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// 单次 LaTeX 调用的墙钟上限（秒）。
pub(crate) const TIMEOUT_SECONDS: u64 = 45;

/// 一次 LaTeX 编译的观测结果。
#[derive(Default)]
pub(crate) struct LatexRun {
    /// 是否成功产出 PDF。
    pub(crate) ok: bool,
    /// 诊断（编译错误行）。
    pub(crate) diagnostics: Vec<String>,
    /// 日志尾部，供证据打印。
    pub(crate) log_tail: String,
    /// `.aux` 里的 `\newlabel`：标签 →（编号，页码）。
    pub(crate) labels: BTreeMap<String, (String, u64)>,
    /// 页数。
    pub(crate) pages: usize,
    /// 逐页文本（`pdftotext -f i -l i`）。
    pub(crate) text_pages: Vec<String>,
    /// 链接：(所在页, 目标锚)（`pdftohtml -xml`）。
    pub(crate) link_targets: Vec<(u64, String)>,
}

/// 在 `dir` 里编译 `main.tex`。
pub(crate) fn compile(dir: &Path, main: &str) -> LatexRun {
    let mut run = LatexRun::default();
    // 用 `timeout` 兜住病态输入（例如递归宏）：不能让它挂住整个构建。
    let output = Command::new("timeout")
        .args([
            &format!("{TIMEOUT_SECONDS}s"),
            "xelatex",
            "-no-shell-escape",
            "-interaction=nonstopmode",
            "-halt-on-error",
            "-file-line-error",
            &format!("{main}.tex"),
        ])
        .current_dir(dir)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            run.diagnostics.push(format!("无法启动 xelatex：{error}"));
            return run;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    run.ok = output.status.success() && dir.join(format!("{main}.pdf")).exists();
    for line in stdout.lines().chain(stderr.lines()) {
        if let Some(rest) = line.strip_prefix("! ") {
            run.diagnostics.push(rest.to_string());
        } else if !run.ok && line.starts_with("./") && line.contains(".tex:") {
            // `-file-line-error` 形式：./main.tex:12: 错误信息
            run.diagnostics.push(line.to_string());
        }
    }
    if !run.ok && run.diagnostics.is_empty() {
        run.diagnostics.push(match output.status.code() {
            Some(124) => format!("xelatex 超过 {TIMEOUT_SECONDS}s 上限被终止"),
            Some(code) => format!("xelatex 非零退出（{code}）"),
            None => "xelatex 被信号终止".to_string(),
        });
    }
    run.log_tail = stdout
        .lines()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");

    let aux = dir.join(format!("{main}.aux"));
    if let Ok(text) = std::fs::read_to_string(&aux) {
        run.labels = parse_aux(&text);
    }
    let pdf = dir.join(format!("{main}.pdf"));
    if run.ok && pdf.exists() {
        run.pages = pdf_pages(&pdf);
        for index in 1..=run.pages {
            run.text_pages.push(pdf_page_text(&pdf, index));
        }
        run.link_targets = pdf_links(&pdf);
    }
    run
}

/// 解析 `.aux` 里的 `\newlabel{name}{{number}{page}...}`。
pub(crate) fn parse_aux(text: &str) -> BTreeMap<String, (String, u64)> {
    let mut labels = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("\\newlabel{") else {
            continue;
        };
        let Some((name, after)) = rest.split_once("}") else {
            continue;
        };
        let body = after.trim_start_matches('{');
        // {{number}{page}{caption}{anchor}{}}
        let mut parts = body.split('}');
        let number = parts
            .next()
            .map(|value| value.trim_start_matches('{').to_string())
            .unwrap_or_default();
        let page = parts
            .next()
            .map(|value| value.trim_start_matches('{').to_string())
            .unwrap_or_default();
        if let Ok(page) = page.parse::<u64>() {
            labels.insert(name.to_string(), (number, page));
        }
    }
    labels
}

/// PDF 页数（`pdfinfo`）。
pub(crate) fn pdf_pages(pdf: &Path) -> usize {
    let output = Command::new("pdfinfo").arg(pdf).output();
    let Ok(output) = output else { return 0 };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .find_map(|line| line.strip_prefix("Pages:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0)
}

/// 第 `page` 页的文本（`pdftotext`）。
pub(crate) fn pdf_page_text(pdf: &Path, page: usize) -> String {
    let output = Command::new("pdftotext")
        .args(["-f", &page.to_string(), "-l", &page.to_string()])
        .arg(pdf)
        .arg("-")
        .output();
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => String::new(),
    }
}

/// 链接列表（`pdftohtml -xml` 的 `<a href="...#锚">`），带所在页。
pub(crate) fn pdf_links(pdf: &Path) -> Vec<(u64, String)> {
    let output = Command::new("pdftohtml")
        .args(["-xml", "-stdout", "-i", "-q"])
        .arg(pdf)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let mut links = Vec::new();
    let mut page = 1u64;
    let mut rest = text.as_str();
    while !rest.is_empty() {
        let next_page = rest.find("<page number=\"");
        let next_link = rest.find("<a href=\"");
        match (next_page, next_link) {
            (Some(page_at), Some(link_at)) if page_at < link_at => {
                let after = &rest[page_at + 14..];
                let end = after.find('"').unwrap_or(0);
                page = after[..end].parse::<u64>().unwrap_or(page);
                rest = &after[end..];
            }
            (_, Some(link_at)) => {
                let after = &rest[link_at + 9..];
                let end = after.find('"').unwrap_or(0);
                links.push((page, after[..end].to_string()));
                rest = &after[end..];
            }
            _ => break,
        }
    }
    links
}
