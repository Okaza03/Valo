//! The player's seek bar: click or drag to seek, the time under the pointer
//! shown above it.
use iced::alignment;
use iced::widget::canvas::{self, Event, Frame, Geometry, Path, Text};
use iced::{mouse, Color, Point, Rectangle, Renderer, Size, Theme};

use super::style;
use crate::util::fmt_time;

/// Room above the bar for the time under the pointer.
pub const ABOVE: f32 = 24.0;
/// The bar's own height (where it reacts to the pointer).
pub const HIT: f32 = 16.0;

pub struct SeekBar<M> {
    /// Where playback is (or where a drag would drop it).
    pub shown: f64,
    pub dur: f64,
    /// Bar height, resting and hovered.
    pub thickness: (f32, f32),
    pub played: Color,
    pub knob: Option<Color>,
    /// The button row layout always shows the knob.
    pub always_knob: bool,
    pub on_drag: fn(f64) -> M,
    pub on_drop: M,
}

#[derive(Default)]
pub struct State {
    dragging: bool,
}

impl<M: Clone> SeekBar<M> {
    fn bar(bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: 0.0,
            y: ABOVE,
            width: bounds.width,
            height: HIT,
        }
    }

    /// The time at `x` (relative to the widget).
    fn at(&self, x: f32, width: f32) -> f64 {
        f64::from((x / width.max(1.0)).clamp(0.0, 1.0)) * self.dur
    }
}

impl<M: Clone> canvas::Program<M> for SeekBar<M> {
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<M>> {
        let local = cursor.position_in(bounds);
        let on_bar = local.is_some_and(|p| Self::bar(bounds).contains(p));
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if on_bar && self.dur > 0.0 =>
            {
                state.dragging = true;
                let x = local.map_or(0.0, |p| p.x);
                Some(
                    canvas::Action::publish((self.on_drag)(self.at(x, bounds.width))).and_capture(),
                )
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                // follow the pointer even outside the bar
                let x = cursor.position().map_or(0.0, |p| p.x - bounds.x);
                Some(canvas::Action::publish((self.on_drag)(
                    self.at(x, bounds.width),
                )))
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                Some(canvas::Action::request_redraw())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                Some(canvas::Action::publish(self.on_drop.clone()).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let bar = Self::bar(bounds);
        let hover = cursor
            .position_in(bounds)
            .filter(|p| bar.contains(*p))
            .map(|p| p.x);
        let hot = hover.is_some() || state.dragging;
        let th = if hot {
            self.thickness.1
        } else {
            self.thickness.0
        };
        let cy = bar.y + bar.height / 2.0;
        let track = |w: f32| {
            Path::rounded_rectangle(
                Point::new(0.0, cy - th / 2.0),
                Size::new(w, th),
                (th / 2.0).into(),
            )
        };
        f.fill(&track(bar.width), style::white(60));
        let frac = if self.dur > 0.0 {
            (self.shown / self.dur).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let played_x = bar.width * frac;
        if played_x > 0.0 {
            f.fill(&track(played_x), self.played);
        }
        if let Some(k) = self.knob {
            if hot || self.always_knob {
                let r = if hot { 8.0 } else { 6.5 };
                f.fill(&Path::circle(Point::new(played_x, cy), r), k);
            }
        }
        if let Some(x) = hover.filter(|_| self.dur > 0.0) {
            f.fill_text(Text {
                content: fmt_time(self.at(x, bounds.width)),
                position: Point::new(x.clamp(20.0, bounds.width - 20.0), ABOVE - 6.0),
                color: Color::WHITE,
                size: 12.0.into(),
                font: style::SANS,
                align_x: iced::widget::text::Alignment::Center,
                align_y: alignment::Vertical::Bottom,
                ..Text::default()
            });
        }
        vec![f.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _: &State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match cursor.position_in(bounds) {
            Some(p) if Self::bar(bounds).contains(p) => mouse::Interaction::Pointer,
            _ => mouse::Interaction::default(),
        }
    }
}
