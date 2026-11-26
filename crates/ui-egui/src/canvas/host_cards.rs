//! Comments beside the page as Google Docs shows them, under a host's [`MarginLook`] (StarDocs
//! 0009): one card per thread right of the page, level with the text it is about, and no
//! markup column or leader lines (the layout fills the commented text instead,
//! [`wordcraft_layout::display::CommentFill`]). The selected card (clicked, or the comment the
//! caret is in) sits exactly level with its text and the others make way above and below it; it
//! is the Comments pane's card, with its fields, Reply and Resolve. A window too narrow for the
//! cards shows a comment icon at the page's edge per thread, and the clicked one's card over the
//! page under its line.

use std::collections::HashMap;

use egui::{Rect, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_layout::DocLayout;

use super::balloons::{deselect_balloon, select_balloon};
use super::geometry::shown_comment;
use super::pages::page_screen_scale;
use crate::WordApp;
use crate::host_look::{CommentsLook, MarginLook};

/// Space between cards, and a card's height before it has been drawn once.
const CARD_GAP: f32 = 8.0;
const FIRST_HEIGHT: f32 = 96.0;
/// The comment icons of a narrow window.
const ICON: f32 = 28.0;

/// One thread's card: its comment, where its text is on screen, and where the caret goes when
/// it is clicked.
struct Thread {
    id: u32,
    page: usize,
    /// The commented line's top and bottom (screen).
    top: f32,
    bottom: f32,
    pos: wordcraft_doc::Pos,
}

fn heights_key(app: &WordApp) -> egui::Id {
    egui::Id::unique(("host_card_heights", app.session.document_id()))
}

/// The cards (or icons) for every thread with an anchor on a page in `rects`.
pub(super) fn host_cards(app: &mut WordApp, ui: &mut Ui, rects: &[Rect], layout: &DocLayout, scale: f32, view: Rect) {
    app.canvas.balloon_rects.clear();
    let Some(look) = app.host_look.comments.clone() else { return };
    let Some(margin) = look.margin.clone() else { return };
    if app.canvas.balloon.is_some_and(|id| !app.session.doc.comments.contains_key(&id)) {
        deselect_balloon(app);
    }
    // A comment just made (Insert › Comment) is selected so its card opens for typing.
    if let Some(id) = crate::panes::focused_comment(app)
        && app.canvas.balloon != Some(id)
        && app.session.doc.comments.contains_key(&id)
    {
        select_balloon(app, id);
    }
    let list: Vec<Value> = app.session.run("review.comments", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let is_root = |c: &Value| c.get("parent").is_none_or(Value::is_null);
    let mut threads = Vec::new();
    for (id, pos) in wordcraft_engine::cmd::review::comment_list(&app.session) {
        let Some(pos) = pos else { continue };
        if !list.iter().any(|c| is_root(c) && c.get("id").and_then(Value::as_u64) == Some(u64::from(id))) {
            continue;
        }
        let Some(caret) = layout.caret_on(&pos, app.session.page_hint) else { continue };
        let Some(pr) = rects.get(caret.page) else { continue };
        let s = layout.pages.get(caret.page).map_or(scale, |p| page_screen_scale(*pr, p, scale));
        let top = pr.min.y + caret.top * s;
        threads.push(Thread { id, page: caret.page, top, bottom: top + caret.height * s, pos });
    }
    threads.sort_by(|a, b| a.top.total_cmp(&b.top));
    let active = shown_comment(app).filter(|id| threads.iter().any(|t| t.id == *id));
    // Only the pages that have a thread count: with pages side by side the last page may be off
    // to the right, and then the cards beside the first are still the right thing (StarDocs 0017).
    let wide = threads.iter().filter_map(|t| rects.get(t.page)).all(|pr| view.max.x - pr.max.x >= margin.card_width + margin.gap + 8.0);
    let mut clicked: Option<usize> = None;
    if wide {
        let mut heights: HashMap<u32, f32> = ui.data(|d| d.get_temp(heights_key(app))).unwrap_or_default();
        let hs: Vec<f32> = threads.iter().map(|t| heights.get(&t.id).copied().unwrap_or(FIRST_HEIGHT)).collect();
        // Each card as near its text as the cards around it allow; the selected one exactly.
        let mut tops: Vec<f32> = threads.iter().map(|t| t.top).collect();
        let first = active.and_then(|a| threads.iter().position(|t| t.id == a)).unwrap_or(0);
        for i in first + 1..threads.len() {
            tops[i] = tops[i].max(tops[i - 1] + hs[i - 1] + CARD_GAP);
        }
        for i in (0..first).rev() {
            tops[i] = tops[i].min(tops[i + 1] - hs[i] - CARD_GAP);
        }
        for (i, t) in threads.iter().enumerate() {
            let Some(pr) = rects.get(t.page) else { continue };
            let at = pos2(pr.max.x + margin.gap, tops[i]);
            let shown = Rect::from_min_size(at, vec2(margin.card_width, hs[i]));
            if !shown.intersects(view.expand(200.0)) {
                continue;
            }
            let selected = active == Some(t.id);
            if card(app, ui, &look, &margin, &list, t.id, at, selected, &mut heights) {
                clicked = Some(i);
            }
        }
        ui.data_mut(|d| d.insert_temp(heights_key(app), heights));
    } else {
        // Narrow: an icon per thread at the page's edge (inside its right margin when even the
        // icon has no room beside it), and the selected thread's card over the page.
        let mut last: Option<(usize, f32)> = None;
        for (i, t) in threads.iter().enumerate() {
            let Some(pr) = rects.get(t.page) else { continue };
            let x = if view.max.x - pr.max.x >= ICON + 8.0 { pr.max.x + 4.0 } else { pr.max.x - ICON - 4.0 };
            // Threads on one line get one icon (the first), as Docs shows a count there.
            if last.is_some_and(|(p, y)| p == t.page && (y - t.top).abs() < 2.0) {
                continue;
            }
            last = Some((t.page, t.top));
            let r = Rect::from_min_size(pos2(x, t.top), vec2(ICON, ICON));
            let resp = ui.interact(r, ui.scope_id().with(("host_comment_icon", t.id)), Sense::click());
            let on = active == Some(t.id);
            if resp.hovered() || on {
                ui.painter().circle_filled(r.center(), ICON / 2.0, margin.highlight);
            }
            let s = ICON * 0.7;
            crate::icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(s, s)), "comment", margin.icon_color, margin.icon_color);
            app.canvas.balloon_rects.push((t.id, r));
            if resp.on_hover_text(tl!("Show comment")).clicked() {
                clicked = Some(i);
            }
        }
        if let Some(t) = active.and_then(|a| threads.iter().find(|t| t.id == a)) {
            let w = margin.card_width.min(view.width() - 16.0);
            let at = pos2((view.max.x - w - 8.0).max(view.min.x + 8.0), t.bottom + 4.0);
            let mut heights = HashMap::new();
            let m = MarginLook { card_width: w, ..margin.clone() };
            card(app, ui, &look, &m, &list, t.id, at, true, &mut heights);
        }
    }
    if let Some(i) = clicked
        && let Some(t) = threads.get(i)
    {
        select_balloon(app, t.id);
        app.session.sel = wordcraft_engine::Selection::caret(t.pos.clone());
    }
}

/// One thread's card at `at`: the Comments pane's card when `selected`, else a short one
/// (author, time, the text's first lines, how many replies) that selects the thread when
/// clicked. Notes its height in `heights`; true when clicked.
#[allow(clippy::too_many_arguments, reason = "one card's inputs, drawn in one place")]
fn card(
    app: &mut WordApp,
    ui: &mut Ui,
    look: &CommentsLook,
    margin: &MarginLook,
    list: &[Value],
    id: u32,
    at: egui::Pos2,
    selected: bool,
    heights: &mut HashMap<u32, f32>,
) -> bool {
    let Some(c) = list.iter().find(|c| c.get("id").and_then(Value::as_u64) == Some(u64::from(id))) else { return false };
    let replies: Vec<&Value> = list.iter().filter(|r| r.get("parent").and_then(Value::as_u64) == Some(u64::from(id))).collect();
    let last = heights.get(&id).copied().unwrap_or(FIRST_HEIGHT);
    let area = Rect::from_min_size(at, vec2(margin.card_width, last));
    // The card's own click area goes in first, so its buttons and fields (drawn after, on top)
    // keep their clicks and the rest of the card takes the others.
    let resp = ui.interact(area, ui.scope_id().with(("host_card", id)), if selected { Sense::hover() } else { Sense::click() });
    let mut child = ui.new_child(
        egui::UiBuilder::new().max_rect(Rect::from_min_size(at, vec2(margin.card_width, 20_000.0))).layout(egui::Layout::top_down(egui::Align::Min)),
    );
    if selected {
        crate::panes::host_card(app, &mut child, look, margin.active_card, c, &replies);
    } else {
        let resolved = c.get("resolved").and_then(Value::as_bool).unwrap_or(false);
        look.card.show(&mut child, |ui| {
            ui.set_width(ui.available_width());
            crate::panes::host_card_header(app, ui, look, c, true, resolved);
            ui.add_space(4.0);
            let text = c.get("text").and_then(Value::as_str).unwrap_or("").trim().to_string();
            let mut job = egui::text::LayoutJob::simple(text, look.text_font.clone(), look.author_color, ui.available_width());
            job.wrap.max_rows = 3;
            ui.label(job);
            if !replies.is_empty() {
                ui.add_space(4.0);
                let n = replies.len();
                let count = n.to_string();
                let words = if n == 1 { tl!("1 reply").to_string() } else { crate::i18n::fmt(tl!("{n} replies"), &[("n", count.as_str())]) };
                ui.label(egui::RichText::new(words).font(look.meta_font.clone()).color(look.buttons.fill));
            }
        });
    }
    let drawn = child.min_rect();
    heights.insert(id, drawn.height());
    app.canvas.balloon_rects.push((id, drawn));
    if resp.hovered() && !selected {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}
