//! Marks drawn over artwork, as in egui: the WATCHED sash across a poster's
//! top right corner, and the red MOVING band while an admin moves a title's
//! files (`ui::watched_sash`, `ui::moving_tape`).
use std::f32::consts::{FRAC_PI_4, FRAC_PI_6};

use iced::alignment::Vertical;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{canvas as canvas_widget, Canvas};
use iced::{mouse, Color, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::iced_ui::style::{self, c, BLACK, BOLD};
use crate::ui::theme;

/// The WATCHED sash over `w`×`h` art, `fade` 0 to 1.
pub fn sash<'a, M: 'a>(w: f32, h: f32, fade: f32) -> Canvas<Sash, M> {
    canvas_widget(Sash { fade }).width(w).height(h)
}

pub struct Sash {
    fade: f32,
}

impl<M> canvas::Program<M> for Sash {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        if self.fade <= 0.0 {
            return vec![f.into_geometry()];
        }
        let k = (bounds.width / 150.0).clamp(0.8, 1.6);
        // the band runs between two diagonals, `a` and `a + w` in from the corner
        let (a, w) = (34.0 * k, 18.0 * k);
        let right = bounds.width;
        let band = Path::new(|b| {
            b.move_to(Point::new(right - a - w, 0.0));
            b.line_to(Point::new(right - a, 0.0));
            b.line_to(Point::new(right, a));
            b.line_to(Point::new(right, a + w));
            b.close();
        });
        f.fill(&band, c(theme::ACCENT()).scale_alpha(self.fade));
        f.stroke(
            &band,
            Stroke::default()
                .with_width(1.0)
                .with_color(style::black((60.0 * self.fade) as u8)),
        );
        let mid = (a + w / 2.0) / 2.0;
        f.with_save(|f| {
            f.translate(Vector::new(right - mid, mid));
            f.rotate(FRAC_PI_4);
            f.fill_text(Text {
                content: "WATCHED".to_owned(),
                position: Point::ORIGIN,
                color: Color::WHITE.scale_alpha(self.fade),
                size: Pixels(9.0 * k),
                font: BOLD,
                align_x: iced::widget::text::Alignment::Center,
                align_y: Vertical::Center,
                ..Text::default()
            });
        });
        vec![f.into_geometry()]
    }
}

/// The MOVING band over `w`×`h` art with corners `radius` round.
pub fn tape<'a, M: 'a>(w: f32, h: f32, radius: f32) -> Canvas<Tape, M> {
    canvas_widget(Tape { radius }).width(w).height(h)
}

pub struct Tape {
    radius: f32,
}

impl<M> canvas::Program<M> for Tape {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let size = bounds.size();
        f.fill(
            &Path::rounded_rectangle(Point::ORIGIN, size, self.radius.into()),
            style::black(130),
        );
        // a poster's width; on wide art its height, so the band stays a
        // band, and no bigger than on a large poster
        let w = size.width.min(size.height).min(420.0);
        let len = (size.width.hypot(size.height)) / 2.0 + w;
        let h = w * 0.13;
        let band = |dy: f32, half: f32| {
            Path::rectangle(
                Point::new(-len, dy - half),
                Size::new(2.0 * len, 2.0 * half),
            )
        };
        f.with_save(|f| {
            f.translate(Vector::new(size.width / 2.0, size.height / 2.0));
            f.rotate(-FRAC_PI_6);
            f.fill(&band(w * 0.02, h), style::black(90));
            f.fill(&band(0.0, h), Color::from_rgb8(0xd4, 0x20, 0x2b));
            let line = Stroke::default()
                .with_width((w * 0.012).max(1.0))
                .with_color(Color::WHITE);
            for side in [-1.0, 1.0] {
                let y = side * h * 0.78;
                f.stroke(&Path::line(Point::new(-len, y), Point::new(len, y)), line);
            }
            f.fill_text(Text {
                content: "MOVING".to_owned(),
                position: Point::ORIGIN,
                color: Color::WHITE,
                size: Pixels(w * 0.17),
                font: BLACK,
                align_x: iced::widget::text::Alignment::Center,
                align_y: Vertical::Center,
                ..Text::default()
            });
        });
        vec![f.into_geometry()]
    }
}
