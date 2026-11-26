//! StarDocs fork changes: Viewing (0005), checklists (0006; ticked items drawn grey, 0013) and Commenting (0010).

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

#[test]
fn commenting_allows_comments_and_nothing_else() {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "Hello world"}));
    assert_eq!(run(&mut s, "view.commenting", json!({"value": true}))["value"], true);
    assert_eq!(run(&mut s, "view.state", json!({}))["commenting"], true);
    let err = s.run("text.insert", &json!({"text": "x"})).unwrap_err().to_string();
    assert!(err.contains("only comment"), "{err}");
    assert!(s.run("format.bold", &json!({})).is_err());
    assert!(s.run("edit.undo", &json!({})).is_err(), "no undo while commenting");
    assert!(s.why_disabled("text.insert").is_some());
    assert!(s.why_disabled("review.newComment").is_none());
    // Suggesting is a change too.
    assert!(s.run("review.trackChanges", &json!({"value": true})).is_err());
    run(&mut s, "select.all", json!({}));
    run(&mut s, "review.newComment", json!({"text": "Looks good"}));
    assert_eq!(s.doc.comments.len(), 1);
    assert_eq!(body_text(&s), ["Hello world"], "the text is untouched");
    assert!(s.why_disabled("file.save").is_none());
    assert!(s.doc.settings.protection.is_none(), "nothing written into the document");
    let back: crate::ViewState = serde_json::from_value(run(&mut s, "view.state", json!({}))).unwrap();
    assert!(!back.commenting, "never read back");
    run(&mut s, "file.new", json!({}));
    assert!(!s.view.commenting, "for the document it was chosen for");
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

/// The colours the glyph runs holding `word` are drawn in.
fn ink_of(s: &mut Session, word: &str) -> Vec<wordcraft_doc::props::Rgb> {
    let layout = s.layout();
    wordcraft_layout::display::page_display(&s.doc, &layout.pages[0], &Default::default())
        .into_iter()
        .filter_map(|d| match d {
            wordcraft_layout::display::Draw::Glyphs { text, color, .. } if text.contains(word) => Some(color),
            _ => None,
        })
        .collect()
}

#[test]
fn ticked_checklist_items_draw_grey_and_unticking_restores() {
    use wordcraft_doc::props::{Rgb, TextColor};
    const GREY: Rgb = Rgb(0x80, 0x80, 0x80);
    const RED: Rgb = Rgb(0xC0, 0x00, 0x00);
    const FADED_RED: Rgb = Rgb(0xE0, 0x80, 0x80);
    // (word, ticked, nested (indented), red text, colour it is drawn in)
    let rows: [(&str, bool, bool, bool, Rgb); 6] = [
        ("Plain", false, false, false, Rgb::BLACK),
        ("Done", true, false, false, GREY),
        ("Nested", false, true, false, Rgb::BLACK),
        ("Deep", true, true, false, GREY),
        ("Rose", false, false, true, RED),
        ("Faded", true, false, true, FADED_RED),
    ];
    let mut s = s();
    for (i, (word, ..)) in rows.iter().enumerate() {
        if i > 0 {
            run(&mut s, "text.newParagraph", json!({}));
        }
        run(&mut s, "text.insert", json!({"text": word}));
    }
    run(&mut s, "select.all", json!({}));
    run(&mut s, "list.checklist", json!({}));
    let paths = s.doc.para_paths(StoryRef::Body);
    let tick = |s: &mut Session, i: usize| {
        let r = item_box(&s.doc, StoryRef::Body, &paths[i]).expect("an item");
        run(s, "developer.toggleCheckBox", json!({"at": serde_json::to_value(r.content_start()).unwrap()}));
    };
    for (i, &(_, ticked, nested, red, _)) in rows.iter().enumerate() {
        let len = s.doc.para(StoryRef::Body, &paths[i]).unwrap().len();
        let r = item_box(&s.doc, StoryRef::Body, &paths[i]).expect("an item");
        if red {
            let a = wordcraft_doc::Pos::new(StoryRef::Body, paths[i].clone(), r.end.off + wordcraft_doc::para::OBJ.len_utf8() + 1);
            s.doc.format_range(&a, &wordcraft_doc::Pos { off: len, ..a.clone() }, &|c| c.color = Some(TextColor::Rgb(RED))).unwrap();
        }
        if nested {
            s.sel = crate::Selection::caret(wordcraft_doc::Pos::new(StoryRef::Body, paths[i].clone(), len));
            run(&mut s, "para.indent", json!({}));
            assert!(item_box(&s.doc, StoryRef::Body, &paths[i]).is_some(), "still an item when nested");
        }
        if ticked {
            tick(&mut s, i);
        }
    }
    for (i, &(word, ticked, _, _, want)) in rows.iter().enumerate() {
        let inks = ink_of(&mut s, word);
        assert!(!inks.is_empty(), "{word}: not drawn");
        assert!(inks.iter().all(|&c| c == want), "{word}: drawn {inks:?}, want {want:?}");
        assert_eq!(s.doc.checklist_ticked(StoryRef::Body, &paths[i]), ticked, "{word}");
        assert_eq!(struck(&s, i), ticked, "{word}");
    }
    // Unticking: the item's own colour again, no strike.
    for (i, &(word, ticked, _, red, _)) in rows.iter().enumerate() {
        if ticked {
            tick(&mut s, i);
            assert!(!struck(&s, i), "{word}");
            let want = if red { RED } else { Rgb::BLACK };
            assert!(ink_of(&mut s, word).iter().all(|&c| c == want), "{word} after unticking");
        }
    }
}

/// Suggesting mode (StarDocs 0015): the caret steps over text already marked deleted, so each
/// Delete / Backspace / Ctrl+Delete / Ctrl+Backspace marks the next piece, not the same one again.
fn suggesting(text: &str, off: usize) -> Session {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": text}));
    s.doc.settings.track_changes = true;
    caret_at(&mut s, off);
    s
}

fn caret_at(s: &mut Session, off: usize) {
    let mut p = s.sel.focus.clone();
    p.off = off;
    s.sel = crate::Selection::caret(p);
}

fn final_text(s: &Session) -> String {
    let path = s.doc.para_paths(StoryRef::Body)[0].clone();
    s.doc.para(StoryRef::Body, &path).unwrap().final_text()
}

#[test]
fn repeated_delete_marks_each_next_character() {
    let mut s = suggesting("Hello", 0);
    for _ in 0..6 {
        run(&mut s, "text.delete", json!({}));
    }
    assert_eq!(body_text(&s)[0], "Hello", "suggested deletions keep the text");
    assert_eq!(final_text(&s), "");
    assert_eq!(s.sel.focus.off, 5);
}

#[test]
fn delete_steps_over_an_earlier_suggested_deletion() {
    let mut s = suggesting("Hello", 0);
    run(&mut s, "text.delete", json!({}));
    caret_at(&mut s, 0);
    run(&mut s, "text.delete", json!({}));
    assert_eq!(final_text(&s), "llo");
}

#[test]
fn repeated_backspace_marks_each_previous_character() {
    let mut s = suggesting("Hello", 5);
    for _ in 0..6 {
        run(&mut s, "text.backspace", json!({}));
    }
    assert_eq!(body_text(&s)[0], "Hello");
    assert_eq!(final_text(&s), "");
    let mut s = suggesting("Hello", 5);
    run(&mut s, "text.backspace", json!({}));
    caret_at(&mut s, 5);
    run(&mut s, "text.backspace", json!({}));
    assert_eq!(final_text(&s), "Hel");
}

#[test]
fn deleting_across_own_insertion_removes_it_for_real() {
    let mut s = suggesting("Hello", 5);
    run(&mut s, "text.insert", json!({"text": "XY"}));
    for _ in 0..3 {
        run(&mut s, "text.backspace", json!({}));
    }
    assert_eq!(body_text(&s)[0], "Hello", "own insertion gone, then 'o' marked deleted");
    assert_eq!(final_text(&s), "Hell");
    let mut s = suggesting("Hello", 5);
    run(&mut s, "text.insert", json!({"text": "XY"}));
    caret_at(&mut s, 5);
    run(&mut s, "text.delete", json!({}));
    run(&mut s, "text.delete", json!({}));
    assert_eq!(body_text(&s)[0], "Hello");
}

#[test]
fn repeated_ctrl_delete_and_ctrl_backspace_take_the_next_word() {
    let mut s = suggesting("one two three", 0);
    for _ in 0..2 {
        run(&mut s, "text.deleteWordForward", json!({}));
    }
    assert_eq!(final_text(&s), "three");
    assert_eq!(body_text(&s)[0], "one two three");
    let mut s = suggesting("one two three", 13);
    for _ in 0..2 {
        run(&mut s, "text.deleteWordBack", json!({}));
    }
    assert_eq!(final_text(&s), "one ");
}
