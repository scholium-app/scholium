use super::*;
use lopdf::dictionary;

fn pdf(annotation: lopdf::Dictionary) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let ann = doc.add_object(annotation);
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),100.into(),100.into()],
        "Annots" => vec![Object::Reference(ann)]
    });
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages", "Kids"=>vec![Object::Reference(page)], "Count"=>1}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    doc.trailer.set("Root", root);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("synthetic PDF");
    bytes
}
#[test]
fn annotations_outside_link_subset_fail_instead_of_disappearing() {
    for subtype in ["Text", "Widget", "FileAttachment", "RichMedia"] {
        assert!(inspect(&pdf(dictionary! {"Subtype"=>subtype})).is_err());
    }
}
#[test]
fn unsafe_actions_and_uris_fail_but_https_is_preserved() {
    for (scheme, accepted) in [
        ("javascript:alert(1)", false),
        ("file:///etc/passwd", false),
        ("https://example.org/a", true),
    ] {
        let bytes = pdf(
            dictionary! {"Subtype"=>"Link", "Rect"=>vec![1.into(),2.into(),20.into(),30.into()],
            "A"=>dictionary!{"S"=>"URI", "URI"=>Object::string_literal(scheme)}},
        );
        assert_eq!(inspect(&bytes).is_ok(), accepted);
    }
    let bytes = pdf(
        dictionary! {"Subtype"=>"Link", "Rect"=>vec![1.into(),2.into(),20.into(),30.into()],
        "A"=>dictionary!{"S"=>"Launch", "F"=>Object::string_literal("command")}},
    );
    assert!(inspect(&bytes).is_err());
    assert!(inspect(b"not a PDF").is_err());
}
