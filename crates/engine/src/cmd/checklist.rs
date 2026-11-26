//! Checklists (StarDocs 0006), as Google Docs' Format › Bullets & numbering › Checklist: each item
//! is a paragraph that starts with a check box content control tagged [`TAG`] and a space. In a
//! .docx that is Word's own check box control (`w14:checkbox`), so Word, LibreOffice and this app
//! all show a box that ticks on a click. Ticking an item strikes its text through (real
//! formatting, so it survives the trip through any program); Enter at the end of an item starts
//! the next one, and Enter on an empty item ends the list, as in Docs.

use serde_json::Value;
use wordcraft_doc::control::{ContentControl, ControlKind, ControlRange};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Document, InlineObject, Path, Pos, StoryRef};

use super::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

/// The tag that makes a check box at the start of a paragraph a checklist item's box.
pub const TAG: &str = wordcraft_doc::control::CHECKLIST_TAG;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("list.checklist", "Checklist", "Home › Paragraph", checklist)
            .key("Mod+Shift+9")
            .params(r#"{"off"?: bool (take the boxes away; left out, the command turns the selected paragraphs into a checklist, or back when they all are one)}"#),
    ]
}

/// The box a checklist item starts with, when the paragraph at `path` is an item.
pub fn item_box(doc: &Document, story: StoryRef, path: &Path) -> Option<ControlRange> {
    doc.checklist_box(story, path)
}

/// Where an item's text starts: after its box and the space that follows it.
fn text_start(doc: &Document, r: &ControlRange) -> Pos {
    let after = r.end.off + OBJ.len_utf8();
    let space = doc.para_at(&r.end).is_some_and(|p| p.text.get(after..).is_some_and(|t| t.starts_with(' ')));
    Pos { off: after + usize::from(space), ..r.end.clone() }
}

/// Strike an item's text through (ticked) or not.
pub(crate) fn strike_item(s: &mut Session, r: &ControlRange, ticked: bool) -> Result<(), CmdError> {
    let a = text_start(&s.doc, r);
    let len = s.doc.para_at(&a).map(|p| p.len()).unwrap_or(a.off);
    if len > a.off {
        s.doc.format_range(&a, &Pos { off: len, ..a.clone() }, &|c| c.strike = ticked.then_some(true))?;
    }
    Ok(())
}

/// Puts an unticked box and a space at the start of the paragraph at `path`; returns how many
/// bytes went in.
fn add_box(s: &mut Session, story: StoryRef, path: &Path) -> Result<usize, CmdError> {
    let para = s.doc.para(story, path).ok_or_else(|| CmdError::Failed("no paragraph".into()))?;
    let before = para.len();
    let base = CharProps { strike: None, ..super::controls::content_props(&s.doc, para.props_of_char(0)) };
    let mut ctl = ContentControl::new(ControlKind::check_box());
    ctl.id = Some(super::controls::new_id(s));
    ctl.tag = TAG.into();
    let unticked = match &ctl.kind {
        ControlKind::CheckBox { unchecked_char, .. } => unchecked_char.to_string(),
        _ => String::new(),
    };
    let at = Pos::new(story, path.clone(), 0);
    let content = s.doc.insert_object(&at, InlineObject::ControlStart { control: Box::new(ctl) }, &base)?;
    let end = s.doc.insert_text(&content, &unticked, &base)?;
    let after = s.doc.insert_object(&end, InlineObject::ControlEnd, &base)?;
    s.doc.insert_text(&after, " ", &base)?;
    let p = s.doc.para_mut(story, path)?;
    // A checklist is a list of its own: the paragraph leaves a bulleted or numbered list.
    p.props.numbering = None;
    p.touch();
    Ok(s.doc.para(story, path).map(|p| p.len()).unwrap_or(before) - before)
}

/// Takes an item's box (and the space after it) away, and the strike through with it; returns
/// how many bytes went.
fn remove_box(s: &mut Session, r: &ControlRange) -> Result<usize, CmdError> {
    if matches!(r.control.kind, ControlKind::CheckBox { checked: true, .. }) {
        strike_item(s, r, false)?;
    }
    let end = text_start(&s.doc, r);
    s.doc.delete_range(&r.start, &end)?;
    Ok(end.off - r.start.off)
}

/// `list.checklist`: the selected paragraphs become checklist items, or plain paragraphs again
/// when they all are items already (or with `off`).
fn checklist(s: &mut Session, v: &Value) -> CmdResult {
    let (a, b) = s.sel.ordered();
    if a.story != b.story {
        return Err(CmdError::Failed("the selection spans more than one story".into()));
    }
    let story = a.story;
    let paths = s.doc.para_paths_in(&a, &b);
    if paths.is_empty() || s.doc.para_at(&a).is_none() {
        return Err(CmdError::Failed("no paragraph at the selection".into()));
    }
    let all = paths.iter().all(|p| item_box(&s.doc, story, p).is_some());
    let off = p::bool(v, "off").unwrap_or(all);
    let (mut anchor, mut focus) = (s.sel.anchor.clone(), s.sel.focus.clone());
    for path in &paths {
        let shift: isize = match item_box(&s.doc, story, path) {
            Some(r) if off => -(remove_box(s, &r)? as isize),
            None if !off => add_box(s, story, path)? as isize,
            _ => 0,
        };
        for pos in [&mut anchor, &mut focus] {
            if pos.path == *path {
                pos.off = pos.off.saturating_add_signed(shift);
            }
        }
    }
    let clamp = |s: &Session, p: Pos| s.doc.clamp(&p);
    s.sel = Selection { anchor: clamp(s, anchor), focus: clamp(s, focus) };
    s.goal_x = None;
    sel_result(s)
}

/// Enter in a checklist item: on an empty item the list ends (its box goes), else `enter` makes
/// the paragraph break and the new paragraph gets an unticked box of its own. `None` when the
/// caret isn't in an item, for the plain Enter.
pub(crate) fn enter(s: &mut Session, enter: impl FnOnce(&mut Session) -> Result<(), CmdError>) -> Option<Result<(), CmdError>> {
    let at = s.sel.focus.clone();
    let r = item_box(&s.doc, at.story, &at.path)?;
    if at.off <= r.end.off {
        // At (or in) the box: a plain Enter.
        return None;
    }
    Some((|| {
        let start = text_start(&s.doc, &r);
        let empty = s.sel.is_collapsed() && s.doc.para_at(&start).is_some_and(|p| p.len() <= start.off);
        if empty {
            remove_box(s, &r)?;
            s.sel = Selection::caret(r.start.clone());
            return Ok(());
        }
        enter(s)?;
        let new = s.sel.focus.clone();
        if new.path != at.path && item_box(&s.doc, new.story, &new.path).is_none() {
            // The text carried over from a ticked item isn't ticked any more.
            let len = s.doc.para_at(&new).map(|p| p.len()).unwrap_or(0);
            if len > 0 {
                s.doc.format_range(&Pos { off: 0, ..new.clone() }, &Pos { off: len, ..new.clone() }, &|c| c.strike = None)?;
            }
            let n = add_box(s, new.story, &new.path)?;
            s.sel = Selection::caret(Pos { off: new.off + n, ..new });
        }
        Ok(())
    })())
}
