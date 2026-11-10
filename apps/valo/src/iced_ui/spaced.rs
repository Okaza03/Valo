//! Letter-spaced text, which Iced's text widget can't do: each character is
//! measured and drawn on its own, `spacing` apart. The labels in the Valo
//! style (tabs, buttons, row titles, the status line) are set this way.
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as atext, Paragraph as _};
use iced::advanced::widget::{tree, Tree, Widget};
use iced::alignment::Vertical;
use iced::{mouse, Color, Element, Font, Length, Pixels, Point, Rectangle, Size, Theme};

use super::style;
use crate::ui::theme::{self, Labels};

pub struct Spaced {
    content: String,
    size: f32,
    spacing: f32,
    font: Font,
    color: Option<Color>,
}

impl Spaced {
    pub fn new(content: impl Into<String>, size: f32, spacing: f32, font: Font) -> Self {
        Self {
            content: content.into(),
            size,
            spacing,
            font,
            color: None,
        }
    }

    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

/// A small label (tabs, buttons, section headers) in the current style:
/// letter-spaced monospace capitals for Valo, spaced bold capitals or plain
/// sans for the others.
pub fn mono(t: &str, size: f32) -> Spaced {
    match theme::spec().labels {
        Labels::Mono => Spaced::new(t.to_uppercase(), size, size * 0.14, style::MONO),
        Labels::Caps => Spaced::new(t.to_uppercase(), size * 1.08, size * 0.12, style::BOLD),
        Labels::Sans => {
            let font = if size >= 12.0 {
                style::BOLD
            } else {
                style::SANS
            };
            Spaced::new(t, size * 1.14, 0.0, font)
        }
    }
}

/// A home page row's title ("Continue watching") in the current style.
pub fn heading(t: &str) -> Spaced {
    let spec = theme::spec();
    match spec.labels {
        Labels::Mono => mono(t, spec.row_title),
        _ => Spaced::new(t, spec.row_title, 0.0, style::BOLD),
    }
}

#[derive(Default)]
struct State {
    key: Option<(String, u32, Font)>,
    advances: Vec<f32>,
    height: f32,
}

impl Spaced {
    fn measure<R: atext::Renderer<Font = Font>>(&self, st: &mut State) {
        let key = (self.content.clone(), self.size.to_bits(), self.font);
        if st.key.as_ref() == Some(&key) {
            return;
        }
        let mut buf = [0u8; 4];
        st.advances = self
            .content
            .chars()
            .map(|ch| {
                let p =
                    R::Paragraph::with_text(self.text(ch.encode_utf8(&mut buf), Size::INFINITE));
                let b = p.min_bounds();
                st.height = st.height.max(b.height);
                b.width
            })
            .collect();
        if st.advances.is_empty() {
            let p = R::Paragraph::with_text(self.text(" ", Size::INFINITE));
            st.height = p.min_bounds().height;
        }
        st.key = Some(key);
    }

    fn text<'b>(&self, content: &'b str, bounds: Size) -> atext::Text<&'b str, Font> {
        atext::Text {
            content,
            bounds,
            size: Pixels(self.size),
            line_height: atext::LineHeight::default(),
            font: self.font,
            align_x: atext::Alignment::Left,
            align_y: Vertical::Top,
            shaping: atext::Shaping::Advanced,
            wrapping: atext::Wrapping::None,
        }
    }
}

impl<M, R> Widget<M, Theme, R> for Spaced
where
    R: atext::Renderer<Font = Font>,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn layout(&mut self, tree: &mut Tree, _: &R, limits: &layout::Limits) -> layout::Node {
        let st = tree.state.downcast_mut::<State>();
        self.measure::<R>(st);
        let n = st.advances.len();
        let w = st.advances.iter().sum::<f32>() + self.spacing * n.saturating_sub(1) as f32;
        layout::Node::new(limits.resolve(Length::Shrink, Length::Shrink, Size::new(w, st.height)))
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        _: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let st = tree.state.downcast_ref::<State>();
        let b = layout.bounds();
        let color = self.color.unwrap_or(style.text_color);
        let mut x = b.x;
        for (ch, adv) in self.content.chars().zip(&st.advances) {
            let t = self.text("", Size::new(adv + 1.0, st.height));
            renderer.fill_text(
                t.with_content(ch.to_string()),
                Point::new(x, b.y),
                color,
                *viewport,
            );
            x += adv + self.spacing;
        }
    }
}

impl<'a, M: 'a> From<Spaced> for Element<'a, M> {
    fn from(s: Spaced) -> Self {
        Element::new(s)
    }
}
