//! 临时探针 2：验证生成器要用的 Typst 写法。

mod world;

use typst::foundations::{Label, Value};
use typst::introspection::Introspector;
use typst::utils::PicoStr;
use typst_layout::PagedDocument;
use world::World;

fn main() {
    let source = r#"#set page(width: 320pt, height: 240pt, margin: 24pt, numbering: "1")
#set math.equation(numbering: "(1)")
#set heading(numbering: "1.")
#set figure(numbering: "1")

= 第一节

行内 $ E = m c^2 $ 与独立公式：
$ integral_0^1 x^2 dif x = frac(1, 3) $ <eq:int>
#context [#metadata(str(counter(math.equation).get().first())) <num:eq:int>]

#figure(
  table(
    columns: 2,
    table.header([甲], [乙]),
    [1], [2],
    [3], [4],
  ),
  kind: table,
  caption: [跨页表示例],
) <tab:demo>
#context [#metadata(str(counter(figure.where(kind: table)).get().first())) <num:tab:demo>]

#figure(
  box(width: 120pt, height: 60pt, stroke: 0.3pt)[
    #place(top + left, curve(
      curve.move((6pt, 54pt)),
      curve.line((6pt, 4pt)),
      curve.line((114pt, 4pt)),
      stroke: 0.6pt,
    ))
    #place(top + left, curve(
      curve.move((10pt, 50pt)),
      curve.line((40pt, 30pt)),
      curve.line((80pt, 12pt)),
      stroke: 1pt + blue,
    ))
  ],
  kind: "figure",
  supplement: [图],
  caption: [原生绘图],
) <fig:demo>
#context [#metadata(str(counter(figure.where(kind: "figure")).get().first())) <num:fig:demo>]

#metadata(none) <comp:foreign>

引用：@eq:int，图 @fig:demo，表 @tab:demo，页 #ref(<tab:demo>, form: "page")，
链接 #link(<comp:foreign>)[跳到外语组件]。
"#;
    let world = World::new(source.to_string());
    let warned = typst::compile::<PagedDocument>(&world);
    println!("warnings = {}", warned.warnings.len());
    for w in &warned.warnings {
        println!("  WARN: {}", w.message);
    }
    let doc = match warned.output {
        Ok(doc) => doc,
        Err(errors) => {
            for error in &errors {
                println!("ERROR: {}", error.message);
            }
            return;
        }
    };
    println!("pages = {}", doc.pages().len());
    for (i, page) in doc.pages().iter().enumerate() {
        println!("--- page {} ---", i + 1);
        println!("{}", dump_text(page));
    }
    let intro = doc.introspector();
    for name in ["num:eq:int", "num:tab:demo", "num:fig:demo"] {
        let label = Label::new(PicoStr::intern(name)).expect("非空");
        let value = intro
            .query_label(label)
            .ok()
            .and_then(|content| content.get_by_name("value").ok());
        println!("probe {name} -> {:?}", value.and_then(value_str));
    }
    let mut links = 0;
    for page in doc.pages() {
        count_links(&page.frame, &mut links);
    }
    println!("links = {links}");
}

fn value_str(value: Value) -> Option<String> {
    match value {
        Value::Str(s) => Some(s.to_string()),
        other => Some(format!("{other:?}")),
    }
}

fn dump_text(page: &typst_layout::Page) -> String {
    let mut out = String::new();
    collect(&page.frame, &mut out);
    out
}

fn collect(frame: &typst::layout::Frame, out: &mut String) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Group(group) => collect(&group.frame, out),
            typst::layout::FrameItem::Text(text) => out.push_str(&text.text),
            typst::layout::FrameItem::Link(_, _) => out.push_str("[LINK]"),
            _ => {}
        }
    }
}

fn count_links(frame: &typst::layout::Frame, count: &mut usize) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Group(group) => count_links(&group.frame, count),
            typst::layout::FrameItem::Link(_, _) => *count += 1,
            _ => {}
        }
    }
}
