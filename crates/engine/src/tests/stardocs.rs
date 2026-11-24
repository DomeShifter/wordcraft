//! StarDocs fork changes: Viewing (0005) and checklists (0006).

use serde_json::json;
use wordcraft_doc::StoryRef;
use wordcraft_doc::control::ControlKind;

use crate::Session;
use crate::cmd::checklist::item_box;

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

fn checked(s: &Session, para: usize) -> bool {
    let path = &s.doc.para_paths(StoryRef::Body)[para];
    matches!(item_box(&s.doc, StoryRef::Body, path).expect("an item").control.kind, ControlKind::CheckBox { checked: true, .. })
}

fn struck(s: &Session, para: usize) -> bool {
    let path = &s.doc.para_paths(StoryRef::Body)[para];
    let p = s.doc.para(StoryRef::Body, path).unwrap();
    p.props_of_char(p.len() - 1).strike == Some(true)
}

#[test]
fn checklist_items_tick_strike_and_continue() {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "Milk"}));
    run(&mut s, "list.checklist", json!({}));
    let path = s.doc.para_paths(StoryRef::Body)[0].clone();
    let r = item_box(&s.doc, StoryRef::Body, &path).expect("Milk became an item");
    assert!(body_text(&s)[0].ends_with("☐\u{FFFC} Milk") || body_text(&s)[0].ends_with(" Milk"), "{:?}", body_text(&s));
    // The caret stays after the text it was after.
    assert_eq!(s.sel.focus.off, s.doc.para(StoryRef::Body, &path).unwrap().len());
    // Enter starts the next item, unticked.
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "Eggs"}));
    assert!(!checked(&s, 1));
    assert!(body_text(&s)[1].ends_with(" Eggs"));
    // Ticking strikes the item through; unticking takes it away again.
    let at = serde_json::to_value(r.content_start()).unwrap();
    run(&mut s, "developer.toggleCheckBox", json!({"at": at}));
    assert!(checked(&s, 0) && struck(&s, 0) && !struck(&s, 1));
    // Enter after a ticked item: the new item isn't ticked or struck.
    let len = s.doc.para(StoryRef::Body, &path).unwrap().len();
    s.sel = crate::Selection::caret(wordcraft_doc::Pos::new(StoryRef::Body, path.clone(), len));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "Bread"}));
    assert!(!checked(&s, 1) && !struck(&s, 1), "{:?}", body_text(&s));
    run(&mut s, "developer.toggleCheckBox", json!({"at": at}));
    assert!(!checked(&s, 0) && !struck(&s, 0));
    // Enter on an empty item ends the list.
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.newParagraph", json!({}));
    let paths = s.doc.para_paths(StoryRef::Body);
    assert_eq!(paths.len(), 4, "{:?}", body_text(&s));
    assert!(item_box(&s.doc, StoryRef::Body, &paths[3]).is_none());
    assert_eq!(body_text(&s)[3], "");
    // Toggling the whole list off takes every box away.
    run(&mut s, "select.all", json!({}));
    run(&mut s, "list.checklist", json!({"off": true}));
    assert_eq!(body_text(&s), ["Milk", "Bread", "Eggs", ""]);
}

#[test]
fn a_checklist_round_trips_through_docx() {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "Done"}));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "To do"}));
    run(&mut s, "select.all", json!({}));
    run(&mut s, "list.checklist", json!({}));
    let path = s.doc.para_paths(StoryRef::Body)[0].clone();
    let at = serde_json::to_value(item_box(&s.doc, StoryRef::Body, &path).unwrap().content_start()).unwrap();
    run(&mut s, "developer.toggleCheckBox", json!({"at": at}));
    let bytes = crate::io::save_bytes("c.docx", &s.doc).unwrap();
    let mut back = Session::new(crate::io::open_bytes("c.docx", &bytes).unwrap());
    assert!(checked(&back, 0) && struck(&back, 0));
    assert!(!checked(&back, 1) && !struck(&back, 1));
    // Still a working checklist: unticking clears the strike.
    let path = back.doc.para_paths(StoryRef::Body)[0].clone();
    let at = serde_json::to_value(item_box(&back.doc, StoryRef::Body, &path).unwrap().content_start()).unwrap();
    run(&mut back, "developer.toggleCheckBox", json!({"at": at}));
    assert!(!checked(&back, 0) && !struck(&back, 0));
}
