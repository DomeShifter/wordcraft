//! A host's look for WordCraft's own dialogs and the Comments pane (StarDocs 0008). A host that
//! draws everything else in its own style (its menus, toolbar, share dialog) sets a [`HostLook`]
//! once with [`WordApp::set_host_look`](crate::WordApp::set_host_look), and the dialogs and the
//! Comments pane take its frame, fonts and buttons without each dialog knowing about it. Nothing
//! set (the default) keeps WordCraft's own look exactly as it was.

use egui::{Color32, CornerRadius, FontId, Frame, Response, Sense, Stroke, Ui, Vec2, vec2};

/// What a host can restyle. `None` keeps WordCraft's look for that part.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostLook {
    /// Every dialog (Page Setup, Word Count, Find and Replace, Insert Hyperlink, Table
    /// Properties …): drawn by [`crate::dialogs::show`].
    pub dialog: Option<DialogLook>,
    /// The Comments pane: a header with its own close and add buttons, and comment cards.
    pub comments: Option<CommentsLook>,
}

/// A dialog's frame, title, text sizes and buttons.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogLook {
    /// The window's fill, corner radius, shadow, outline and inner margin.
    pub frame: Frame,
    /// The title, drawn at the top left of the body in place of WordCraft's title bar.
    pub title_font: FontId,
    pub title_color: Color32,
    /// Labels, fields, check boxes and lists.
    pub body_size: f32,
    /// Group headings inside a dialog ("Margins", "Orientation" …).
    pub heading_font: FontId,
    /// Space between rows and between items on a row.
    pub item_spacing: Vec2,
    /// Height of fields, check boxes and list boxes.
    pub row_height: f32,
    /// Fields and drop-downs (StarDocs 0009): outline at rest, under the pointer and while
    /// typing or open, and corner radius.
    pub field_stroke: Stroke,
    pub field_hover: Stroke,
    pub field_focus: Stroke,
    pub field_radius: u8,
    /// Narrowest a dialog is drawn.
    pub min_width: f32,
    /// The close (✕) button's icon colour and round hover fill.
    pub close_color: Color32,
    pub buttons: ButtonLook,
}

/// OK / Cancel style buttons: the default action is a filled pill, the others are text buttons.
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonLook {
    pub font: FontId,
    pub height: f32,
    /// Room left and right of a button's text.
    pub padding: f32,
    /// The filled (default) button: fill and text colour.
    pub fill: Color32,
    pub on_fill: Color32,
    /// Text buttons: text colour, and the wash behind them on hover.
    pub text: Color32,
    pub hover: Color32,
}

/// The Comments pane: panel, header and cards.
#[derive(Clone, Debug, PartialEq)]
pub struct CommentsLook {
    /// The pane's background, behind the cards.
    pub panel_fill: Color32,
    /// The pane's outline against the page area.
    pub panel_stroke: Stroke,
    pub header_font: FontId,
    pub header_color: Color32,
    /// One comment thread: fill, radius, shadow, outline and inner margin.
    pub card: Frame,
    /// Space between cards.
    pub card_gap: f32,
    pub author_font: FontId,
    pub author_color: Color32,
    /// The time under the author, and the empty pane's hint.
    pub meta_font: FontId,
    pub meta_color: Color32,
    /// Comment and reply text.
    pub text_font: FontId,
    /// Icon buttons in the header and on cards (add, resolve, more, close).
    pub icon_color: Color32,
    pub buttons: ButtonLook,
    /// Comments beside the page as cards (StarDocs 0009); `None` keeps WordCraft's balloons.
    pub margin: Option<MarginLook>,
}

/// Comments beside the page the way Google Docs shows them (StarDocs 0009): one card per thread
/// in the space right of the page, level with its text, with no markup column and no leader
/// lines; the commented text is filled instead. The page moves left only as far as the cards
/// need, and a window too narrow for them shows a comment icon at the page's edge per comment,
/// whose card opens over the page when clicked. Cards take [`CommentsLook::card`] and its fonts.
#[derive(Clone, Debug, PartialEq)]
pub struct MarginLook {
    /// A card's width, and the space between the page and the cards.
    pub card_width: f32,
    pub gap: f32,
    /// Behind commented text, and behind the selected comment's text.
    pub highlight: Color32,
    pub highlight_active: Color32,
    /// The selected card's frame (a stronger shadow, as Docs lifts it).
    pub active_card: Frame,
    /// The comment icons of a narrow window.
    pub icon_color: Color32,
}

fn buttons_key() -> egui::Id {
    egui::Id::unique("wordcraft_host_buttons")
}

/// The button look in force while a host-styled dialog or pane is drawn (see [`with_buttons`]).
pub(crate) fn buttons(ctx: &egui::Context) -> Option<ButtonLook> {
    ctx.data(|d| d.get_temp::<ButtonLook>(buttons_key()))
}

/// Draws `add` with `look`'s buttons for [`crate::widgets::primary_button`] and
/// [`text_button`]; `None` leaves WordCraft's.
pub(crate) fn with_buttons<R>(ctx: &egui::Context, look: Option<&ButtonLook>, add: impl FnOnce() -> R) -> R {
    let Some(look) = look else { return add() };
    let before = buttons(ctx);
    ctx.data_mut(|d| d.insert_temp(buttons_key(), look.clone()));
    let r = add();
    ctx.data_mut(|d| {
        if let Some(b) = before {
            d.insert_temp(buttons_key(), b);
        } else {
            d.remove::<ButtonLook>(buttons_key());
        }
    });
    r
}

/// The host's filled pill for `text`, when a host look is in force.
pub(crate) fn primary(ui: &Ui, text: impl Into<String>) -> Option<egui::Button<'static>> {
    let b = buttons(ui.ctx())?;
    let label = egui::RichText::new(text.into()).font(b.font.clone()).color(b.on_fill);
    Some(egui::Button::new(label).fill(b.fill).stroke(Stroke::NONE).corner_radius(CornerRadius::same(255)).min_size(vec2(b.height * 2.0, b.height)))
}

/// A secondary action (Cancel, Close …): a pill-shaped text button under a host look, else
/// WordCraft's plain button.
pub(crate) fn text_button(ui: &mut Ui, text: &str) -> Response {
    let Some(b) = buttons(ui.ctx()) else { return ui.button(text) };
    pill(ui, &b, text, false)
}

/// A pill button in `b`'s look: filled (the default action) or a text button.
pub(crate) fn pill(ui: &mut Ui, b: &ButtonLook, text: &str, filled: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), b.font.clone(), if filled { b.on_fill } else { b.text });
    let size = vec2((galley.size().x + 2.0 * b.padding).max(b.height * 2.0), b.height);
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), text));
    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same(255);
        let hot = r.hovered() || r.has_focus();
        let down = r.is_pointer_button_down_on();
        if filled {
            let fill = if down {
                b.fill.gamma_multiply(0.85)
            } else if hot {
                b.fill.gamma_multiply(0.92)
            } else {
                b.fill
            };
            ui.painter().rect_filled(rect, radius, fill);
        } else if hot || down {
            ui.painter().rect_filled(rect, radius, if down { b.hover.gamma_multiply(1.6) } else { b.hover });
        }
        if r.has_focus() {
            ui.painter().rect_stroke(rect.expand(2.0), radius, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
        }
        let pos = rect.center() - galley.size() / 2.0;
        ui.painter().galley(pos, galley, b.text);
    }
    r
}

/// A round icon button (close, add, resolve, more) `size` across, with a round hover wash.
pub(crate) fn round_icon(ui: &mut Ui, icon: &str, tip: &str, size: f32, color: Color32, hover: Color32) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tip));
    if ui.is_rect_visible(rect) {
        if r.hovered() || r.has_focus() || r.is_pointer_button_down_on() {
            ui.painter().circle_filled(rect.center(), size / 2.0, hover);
        }
        let s = (size * 0.55).round();
        crate::icons::paint(ui.painter(), egui::Rect::from_center_size(rect.center(), vec2(s, s)), icon, color, color);
    }
    r.on_hover_text(tip)
}

/// Sets `ui`'s text sizes and spacing to a host dialog look: labels, fields and lists at
/// `body_size`, roomier rows.
pub(crate) fn apply_body(ui: &mut Ui, look: &DialogLook) {
    let style = ui.style_mut();
    for (ts, font) in style.text_styles.iter_mut() {
        if matches!(ts, egui::TextStyle::Body | egui::TextStyle::Button | egui::TextStyle::Monospace) {
            font.size = look.body_size;
        }
    }
    style.spacing.item_spacing = look.item_spacing;
    style.spacing.interact_size.y = look.row_height;
    style.spacing.button_padding.y = style.spacing.button_padding.y.max((look.row_height - look.body_size * 1.3) / 2.0);
    let w = &mut style.visuals.widgets;
    for (state, stroke) in [(&mut w.inactive, look.field_stroke), (&mut w.hovered, look.field_hover), (&mut w.active, look.field_focus), (&mut w.open, look.field_focus)]
    {
        state.bg_stroke = stroke;
        state.corner_radius = CornerRadius::same(look.field_radius);
        state.expansion = 0.0;
    }
    style.visuals.selection.stroke = look.field_focus;
}

/// The group-heading font for a dialog: the host's, else WordCraft's 12.5 pt semibold.
pub(crate) fn dialog_heading(ui: &Ui) -> FontId {
    ui.ctx().data(|d| d.get_temp::<FontId>(heading_key())).unwrap_or_else(|| crate::theme::semibold(12.5))
}

pub(crate) fn heading_key() -> egui::Id {
    egui::Id::unique("wordcraft_host_dialog_heading")
}
