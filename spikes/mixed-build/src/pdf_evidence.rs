//! 独立 PDF 检查：探针失败必须阻断验收。
use std::path::Path;

/// 用 `pdfimages -list` 数栅格图像；工具失败不能当作零图像。
pub(crate) fn raster_images(pdf: &Path) -> std::io::Result<usize> {
    let output = crate::pdf_sandbox::run(pdf, crate::pdf_sandbox::Probe::Images)?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "pdfimages {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with(|character: char| character.is_ascii_digit())
        })
        .count())
}

#[cfg(test)]
mod tests {
    #[test]
    fn unreadable_pdf_does_not_count_as_vector() {
        // A missing input (or missing tool) is an error, never evidence of zero images.
        assert!(super::raster_images(std::path::Path::new("/dev/null/not-a-pdf")).is_err());
    }
}
