//! 临时冒烟测试：验证 World + 编译 + SVG 导出 + PDF 图片嵌入 API。

mod world;

use typst_layout::PagedDocument;
use world::World;

fn main() {
    // 1) 编译一段含标签、引用、方程的 Typst 源码。
    let source = r#"#set page(width: 300pt, height: 200pt, margin: 20pt)
#set math.equation(numbering: "(1)")
#set heading(numbering: "1.")
= 标题
正文 $ E = m c^2 $ <eq:mass>
见 @eq:mass 与 @sec:later。
#figure([#rect(width: 40pt, height: 20pt)], caption: [一个图]) <fig:x>
见图 @fig:x 与第 @eq:mass 式。
#pagebreak()
== 后一节 <sec:later>
后一节。
"#;
    let world = World::new(source.to_string());
    let warned = typst::compile::<PagedDocument>(&world);
    println!("warnings = {}", warned.warnings.len());
    match &warned.output {
        Ok(doc) => {
            println!("pages = {}", doc.pages().len());
            for (i, page) in doc.pages().iter().enumerate() {
                println!("--- page {} text ---", i + 1);
                println!("{}", dump_text(page));
            }
        }
        Err(errors) => {
            for e in errors {
                println!("ERROR: {}", e.message);
            }
        }
    }
    let Ok(doc) = warned.output else { return };
    // 2) SVG 导出
    let svg = typst_svg::svg(
        &doc.pages()[0],
        &typst_svg::SvgOptions {
            render_bleed: false,
            pretty: true,
        },
    );
    println!("svg head = {}", svg.chars().take(200).collect::<String>());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("out/smoke");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("page1.svg"), &svg);
    println!("svg bytes = {}", svg.len());

    // 3) 嵌入外部 PDF 图片（先造一个最小 PDF：用 out/smoke 里已有的？这里用 rsvg 之后再说）
    //    先用 Typst 自己的 SVG 转成 PDF 不方便，改为检测 Typst 能否读 PDF：留待 bridge 阶段。
}

/// 递归抽取一页里的文本。
fn dump_text(page: &typst_layout::Page) -> String {
    let mut out = String::new();
    collect(&page.frame, 0, &mut out);
    out
}

fn collect(frame: &typst::layout::Frame, depth: usize, out: &mut String) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Group(group) => collect(&group.frame, depth + 1, out),
            typst::layout::FrameItem::Text(text) => {
                if depth == 0 || true {
                    out.push_str(&text.text);
                }
            }
            typst::layout::FrameItem::Link(_, _) => out.push_str("[LINK]"),
            _ => {}
        }
    }
}
