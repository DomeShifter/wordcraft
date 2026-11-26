//! The Comments pane under a host look (StarDocs 0008), drawn like Google Docs' comments: a
//! header with add and close buttons, one card per thread (author, time, text, replies, a reply
//! box), Comment / Cancel on a new comment and Reply / Cancel on a reply being written, and a
//! message when there are no comments. The commands are the same as WordCraft's pane.

use egui::{Align, Color32, RichText, Ui, vec2};
use serde_json::{Value, json};

use super::{comment_editor, commit_comment, commit_reply, drafts, field_id, reply_field};
use crate::WordApp;
use crate::host_look::{CommentsLook, pill, round_icon, with_buttons};

/// Round icon buttons in the header and on cards.
const ICON: f32 = 32.0;
/// The author's initial in a circle at the start of a card.
const AVATAR: f32 = 32.0;

pub(super) fn comments(app: &mut WordApp, ui: &mut Ui, look: &CommentsLook) {
    let hover = look.icon_color.gamma_multiply(0.1);
    ui.horizontal(|ui| {
        ui.label(RichText::new(tl!("Comments")).font(look.header_font.clone()).color(look.header_color));
        ui.with_layout(crate::rtl::far_side(ui, Align::Center), |ui| {
            if round_icon(ui, "close", tl!("Close"), ICON, look.icon_color, hover).clicked() {
                let _ = app.run("view.commentsPane", json!({"value": false}));
            }
            let can_add = crate::widgets::enabled(app, "review.newComment");
            if ui.add_enabled_ui(can_add, |ui| round_icon(ui, "plus", tl!("Add a comment"), ICON, look.icon_color, hover)).inner.clicked() {
                let _ = app.run("review.newComment", json!({"text": ""}));
            }
        });
    });
    ui.add_space(8.0);
    let list: Vec<Value> = app.session.run("review.comments", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let is_root = |c: &Value| c.get("parent").is_none_or(Value::is_null);
    if !list.iter().any(is_root) {
        ui.vertical_centered(|ui| {
            ui.add_space(32.0);
            ui.label(RichText::new(tl!("No comments yet")).font(look.author_font.clone()).color(look.author_color));
            ui.add_space(4.0);
            ui.label(RichText::new(tl!("Select some text, then click + to add a comment.")).font(look.meta_font.clone()).color(look.meta_color));
        });
        return;
    }
    // Show all / only open / only resolved threads (kept per document while it is open).
    let filter_key = egui::Id::unique(("host_comment_filter", app.session.document_id()));
    let mut filter: u8 = ui.data(|d| d.get_temp(filter_key)).unwrap_or(0);
    ui.horizontal(|ui| {
        for (n, label) in [(0u8, tl!("All")), (1, tl!("Open")), (2, tl!("Resolved"))] {
            if ui.selectable_label(filter == n, RichText::new(label).font(look.meta_font.clone())).clicked() {
                filter = n;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(filter_key, filter));
    ui.add_space(6.0);
    app.canvas.pane_card_rects.clear();
    let active = crate::canvas::shown_comment(app);
    let resolved_of = |c: &Value| c.get("resolved").and_then(Value::as_bool).unwrap_or(false);
    with_buttons(&ui.ctx().clone(), Some(&look.buttons), || {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for c in list.iter().filter(|c| is_root(c) && (filter == 0 || (filter == 2) == resolved_of(c))) {
                let id = c.get("id").and_then(Value::as_u64).unwrap_or(0);
                let replies: Vec<&Value> = list.iter().filter(|r| r.get("parent").and_then(Value::as_u64) == Some(id)).collect();
                // A click on the card (not on its buttons or fields, which are drawn above this) moves
                // the caret into its anchored text: the page scrolls there and the text is highlighted.
                let h_key = egui::Id::unique(("host_pane_card_h", app.session.document_id(), id));
                let last: f32 = ui.data(|d| d.get_temp(h_key)).unwrap_or(60.0);
                let area = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), last));
                let hit = ui.interact(area, h_key.with("click"), egui::Sense::click());
                let frame = match (&look.margin, active == u32::try_from(id).ok()) {
                    (Some(m), true) => m.active_card,
                    _ => look.card,
                };
                let top = ui.cursor().min.y;
                card(app, ui, look, frame, c, &replies);
                ui.data_mut(|d| d.insert_temp(h_key, (ui.cursor().min.y - top).max(20.0)));
                let drawn = egui::Rect::from_min_size(area.min, vec2(area.width(), (ui.cursor().min.y - top).max(20.0)));
                app.canvas.pane_card_rects.push((u32::try_from(id).unwrap_or(0), drawn));
                if hit.clicked()
                    && let Some(a) = c.get("anchor").filter(|a| !a.is_null())
                {
                    let _ = app.run("caret.set", json!({"pos": a}));
                }
                ui.add_space(look.card_gap);
            }
        });
    });
}

/// One thread: the comment, its replies, and the reply box (or Comment / Cancel on a new one),
/// in `frame` (the pane's card, or a selected card beside the page, StarDocs 0009).
pub(crate) fn card(app: &mut WordApp, ui: &mut Ui, look: &CommentsLook, frame: egui::Frame, c: &Value, replies: &[&Value]) {
    let id = c.get("id").and_then(Value::as_u64).and_then(|i| u32::try_from(i).ok()).unwrap_or(0);
    let resolved = c.get("resolved").and_then(Value::as_bool).unwrap_or(false);
    let document = app.session.document_id();
    // A comment is "new" from when it is drawn with no text until Comment or Cancel.
    let new_key = egui::Id::unique(("host_comment_new", document, id));
    let empty = c.get("text").and_then(Value::as_str).is_none_or(|t| t.trim().is_empty());
    if empty {
        ui.data_mut(|d| d.insert_temp(new_key, true));
    }
    let is_new = ui.data(|d| d.get_temp::<bool>(new_key)).unwrap_or(false);
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        header(app, ui, look, c, true, resolved);
        ui.add_space(4.0);
        let field = field_id(app, ui, "comment_edit", id);
        comment_editor(app, ui, id, Some(look.text_font.clone()), false);
        if is_new {
            ui.add_space(8.0);
            let typed = drafts(app).edits.get(&id).is_some_and(|t| !t.trim().is_empty()) || !empty;
            button_row(ui, |ui| {
                // Far side first: Comment at the end, Cancel before it.
                if ui.add_enabled_ui(typed, |ui| pill(ui, &look.buttons, tl!("Comment"), true)).inner.clicked() {
                    let _ = commit_comment(app, id);
                    ui.data_mut(|d| d.remove::<bool>(new_key));
                    ui.memory_mut(|m| m.surrender_focus(field));
                }
                if pill(ui, &look.buttons, tl!("Cancel"), false).clicked() {
                    drafts(app).edits.remove(&id);
                    drafts(app).live = None;
                    ui.data_mut(|d| d.remove::<bool>(new_key));
                    let _ = app.run("review.deleteComment", json!({"id": id}));
                }
            });
            return;
        }
        for r in replies {
            let rid = r.get("id").and_then(Value::as_u64).and_then(|i| u32::try_from(i).ok()).unwrap_or(0);
            ui.add_space(8.0);
            header(app, ui, look, r, false, false);
            ui.add_space(2.0);
            comment_editor(app, ui, rid, Some(look.text_font.clone()), false);
        }
        if resolved {
            return;
        }
        ui.add_space(8.0);
        reply_field(app, ui, id, Some(look.text_font.clone()), false, false);
        if drafts(app).replies.get(&id).is_some_and(|t| !t.trim().is_empty()) {
            ui.add_space(6.0);
            button_row(ui, |ui| {
                if pill(ui, &look.buttons, tl!("Reply"), true).clicked() {
                    let _ = commit_reply(app, id);
                }
                if pill(ui, &look.buttons, tl!("Cancel"), false).clicked() {
                    drafts(app).replies.remove(&id);
                }
            });
        }
    });
}

/// A row of pill buttons at the far side, one button tall.
fn button_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.with_layout(crate::rtl::far_side(ui, Align::Center), add);
    });
}

/// Author (bold), time (grey) and, on a thread's first comment, Resolve and the ⋮ menu.
pub(crate) fn header(app: &mut WordApp, ui: &mut Ui, look: &CommentsLook, c: &Value, root: bool, resolved: bool) {
    let id = c.get("id").and_then(Value::as_u64).unwrap_or(0);
    let author = c.get("author").and_then(Value::as_str).unwrap_or("").to_string();
    let when = u32::try_from(id).ok().and_then(|i| app.session.doc.comments.get(&i)).map(crate::local_time::comment_time).unwrap_or_default();
    let hover = look.icon_color.gamma_multiply(0.1);
    ui.horizontal(|ui| {
        avatar(ui, &author, look.buttons.fill);
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new(&author).font(look.author_font.clone()).color(look.author_color));
            let meta = if resolved { format!("{when} · {}", tl!("Resolved")) } else { when };
            ui.label(RichText::new(meta).font(look.meta_font.clone()).color(look.meta_color));
        });
        if !root {
            return;
        }
        ui.with_layout(crate::rtl::far_side(ui, Align::Center), |ui| {
            let more = round_icon(ui, "more", tl!("More options"), ICON, look.icon_color, hover);
            egui::Popup::menu(&more).show(|ui| {
                if ui.button(tl!("Go to")).clicked()
                    && let Some(a) = c.get("anchor").filter(|a| !a.is_null())
                {
                    let _ = app.run("caret.set", json!({"pos": a}));
                }
                if resolved && ui.button(tl!("Reopen")).clicked() {
                    let _ = app.run("review.resolveComment", json!({"id": id}));
                }
                if ui.button(tl!("Delete")).clicked() {
                    let _ = app.run("review.deleteComment", json!({"id": id}));
                }
            });
            if !resolved && round_icon(ui, "check", tl!("Resolve"), ICON, look.buttons.fill, hover).clicked() {
                let _ = app.run("review.resolveComment", json!({"id": id}));
            }
        });
    });
}

/// The author's first letter, white on a filled circle.
fn avatar(ui: &mut Ui, author: &str, fill: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(AVATAR, AVATAR), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), AVATAR / 2.0, fill);
    let initial: String = author.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initial, egui::FontId::proportional(15.0), Color32::WHITE);
}
