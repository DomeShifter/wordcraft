//! StarDocs fork changes: Viewing (0005) .

use serde_json::json;
use wordcraft_doc::StoryRef;

use crate::Session;

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn run(s: &mut Session, id: &str, v: serde_json::Value) -> serde_json::Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn body_text(s: &Session) -> Vec<String> {
    s.doc.para_paths(StoryRef::Body).iter().map(|p| s.doc.para(StoryRef::Body, p).unwrap().text.to_string()).collect()
}

#[test]
fn viewing_refuses_edits_for_the_session_only() {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "Hello"}));
    assert_eq!(run(&mut s, "view.viewing", json!({"value": true}))["value"], true);
    assert_eq!(run(&mut s, "view.state", json!({}))["viewing"], true);
    let err = s.run("text.insert", &json!({"text": "x"})).unwrap_err().to_string();
    assert!(err.contains("open for viewing"), "{err}");
    assert!(s.run("format.bold", &json!({})).is_err());
    assert!(s.run("edit.undo", &json!({})).is_err(), "no undo while viewing");
    assert!(s.why_disabled("text.insert").is_some() && s.why_disabled("format.bold").is_some());
    // Reading, moving, selecting, copying and zooming still work.
    run(&mut s, "select.all", json!({}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "view.zoom", json!({"value": 150}));
    assert!(s.why_disabled("file.save").is_none(), "saving a copy is a File command");
    // Nothing of it is in the document, and nothing is read back from a saved view.
    assert!(s.doc.settings.protection.is_none());
    let bytes = crate::io::save_bytes("v.docx", &s.doc).unwrap();
    assert!(crate::io::open_bytes("v.docx", &bytes).unwrap().settings.protection.is_none());
    let back: crate::ViewState = serde_json::from_value(run(&mut s, "view.state", json!({}))).unwrap();
    assert!(!back.viewing);
    run(&mut s, "view.viewing", json!({"value": false}));
    run(&mut s, "text.insert", json!({"text": "!"}));
    assert_eq!(body_text(&s), ["Hello!"]);
}

#[test]
fn a_new_document_is_not_opened_for_viewing() {
    let mut s = s();
    run(&mut s, "view.viewing", json!({"value": true}));
    run(&mut s, "file.new", json!({}));
    assert!(!s.view.viewing);
    run(&mut s, "text.insert", json!({"text": "ok"}));
}
