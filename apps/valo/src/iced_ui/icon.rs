//! The app's drawn icons (the same shapes as `ui::chrome::draw_icon`), and a spinner.
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{canvas as canvas_widget, Canvas};
use iced::{mouse, Color, Point, Rectangle, Renderer, Size, Theme, Vector};

pub use crate::ui::chrome::Icon;

/// An icon in a `size`-square box.
pub fn icon<'a, M: 'a>(icon: Icon, color: Color, size: f32) -> Canvas<Drawn, M> {
    canvas_widget(Drawn { icon, color })
        .width(size)
        .height(size)
}

pub struct Drawn {
    icon: Icon,
    color: Color,
}

impl<M> canvas::Program<M> for Drawn {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let c = frame.center();
        draw(&mut frame, c, self.icon, self.color);
        vec![frame.into_geometry()]
    }
}

fn v(x: f32, y: f32) -> Vector {
    Vector::new(x, y)
}

fn line(f: &mut Frame, a: Point, b: Point, w: f32, col: Color) {
    f.stroke(&Path::line(a, b), stroke(w, col));
}

fn stroke(w: f32, col: Color) -> Stroke<'static> {
    Stroke::default().with_width(w).with_color(col)
}

fn polyline(f: &mut Frame, pts: &[Point], w: f32, col: Color) {
    let path = Path::new(|b| {
        b.move_to(pts[0]);
        for p in &pts[1..] {
            b.line_to(*p);
        }
    });
    f.stroke(&path, stroke(w, col));
}

fn polygon(pts: &[Point]) -> Path {
    Path::new(|b| {
        b.move_to(pts[0]);
        for p in &pts[1..] {
            b.line_to(*p);
        }
        b.close();
    })
}

/// A rectangle outline centered on `c`.
fn rect(f: &mut Frame, c: Point, w: f32, h: f32, r: f32, sw: f32, col: Color) {
    let path = Path::rounded_rectangle(c + v(-w / 2.0, -h / 2.0), Size::new(w, h), r.into());
    f.stroke(&path, stroke(sw, col));
}

pub fn draw(f: &mut Frame, c: Point, icon: Icon, col: Color) {
    let s = 1.4;
    match icon {
        Icon::Minimize => line(f, c + v(-6.0, 0.0), c + v(6.0, 0.0), s, col),
        Icon::Maximize => rect(f, c, 10.0, 10.0, 0.0, s, col),
        Icon::Restore => {
            rect(f, c + v(-1.5, 1.5), 8.0, 8.0, 0.0, s, col);
            line(f, c + v(-1.5, -4.5), c + v(4.5, -4.5), s, col);
            line(f, c + v(4.5, -4.5), c + v(4.5, 1.5), s, col);
        }
        Icon::Close => {
            line(f, c + v(-6.0, -6.0), c + v(6.0, 6.0), s, col);
            line(f, c + v(6.0, -6.0), c + v(-6.0, 6.0), s, col);
        }
        Icon::Refresh => {
            let pts: Vec<Point> = (0..=24)
                .map(|i| {
                    let a = -0.35 * PI + i as f32 / 24.0 * 1.65 * PI;
                    c + v(a.cos(), a.sin()) * 7.0
                })
                .collect();
            let tip = pts[0];
            polyline(f, &pts, 1.8, col);
            f.fill(
                &polygon(&[tip + v(-4.5, -2.0), tip + v(2.5, -3.5), tip + v(1.0, 3.5)]),
                col,
            );
        }
        Icon::Bookmark | Icon::BookmarkOn => {
            let pts = [
                c + v(-5.0, -7.0),
                c + v(5.0, -7.0),
                c + v(5.0, 7.0),
                c + v(0.0, 3.0),
                c + v(-5.0, 7.0),
            ];
            let path = polygon(&pts);
            if matches!(icon, Icon::BookmarkOn) {
                f.fill(&path, col);
            }
            f.stroke(&path, stroke(1.6, col));
        }
        Icon::Play => f.fill(
            &polygon(&[c + v(-4.0, -6.0), c + v(6.0, 0.0), c + v(-4.0, 6.0)]),
            col,
        ),
        Icon::Pause => {
            for dx in [-3.5, 3.5] {
                f.fill(
                    &Path::rounded_rectangle(
                        c + v(dx - 1.75, -6.5),
                        Size::new(3.5, 13.0),
                        1.0.into(),
                    ),
                    col,
                );
            }
        }
        Icon::ChevronLeft => polyline(
            f,
            &[c + v(2.0, -4.0), c + v(-2.0, 0.0), c + v(2.0, 4.0)],
            s,
            col,
        ),
        Icon::ChevronRight => polyline(
            f,
            &[c + v(-2.0, -4.0), c + v(2.0, 0.0), c + v(-2.0, 4.0)],
            s,
            col,
        ),
        Icon::ChevronUp => polyline(
            f,
            &[c + v(-4.0, 2.0), c + v(0.0, -2.0), c + v(4.0, 2.0)],
            s,
            col,
        ),
        Icon::ChevronDown => polyline(
            f,
            &[c + v(-4.0, -2.0), c + v(0.0, 2.0), c + v(4.0, -2.0)],
            s,
            col,
        ),
        Icon::ArrowRight => {
            line(f, c + v(-5.0, 0.0), c + v(5.0, 0.0), s, col);
            polyline(
                f,
                &[c + v(1.0, -4.0), c + v(5.0, 0.0), c + v(1.0, 4.0)],
                s,
                col,
            );
        }
        Icon::ArrowUp => {
            line(f, c + v(0.0, 5.0), c + v(0.0, -5.0), s, col);
            polyline(
                f,
                &[c + v(-4.0, -1.0), c + v(0.0, -5.0), c + v(4.0, -1.0)],
                s,
                col,
            );
        }
        Icon::ArrowDown => {
            line(f, c + v(0.0, -5.0), c + v(0.0, 5.0), s, col);
            polyline(
                f,
                &[c + v(-4.0, 1.0), c + v(0.0, 5.0), c + v(4.0, 1.0)],
                s,
                col,
            );
        }
        Icon::Star => {
            let pts: Vec<Point> = (0..10)
                .map(|i| {
                    let a = -FRAC_PI_2 + i as f32 * PI / 5.0;
                    let r = if i % 2 == 0 { 6.0 } else { 2.6 };
                    c + v(a.cos(), a.sin()) * r
                })
                .collect();
            f.fill(&polygon(&pts), col);
        }
        Icon::Subtitles => {
            rect(f, c, 16.0, 12.0, 2.0, s, col);
            line(f, c + v(-4.5, 1.0), c + v(-1.5, 1.0), s, col);
            line(f, c + v(0.5, 1.0), c + v(4.5, 1.0), s, col);
            line(f, c + v(-4.5, -2.5), c + v(1.5, -2.5), s, col);
        }
        Icon::Volume | Icon::VolumeMuted => {
            let body = polygon(&[
                c + v(-7.0, -2.5),
                c + v(-4.0, -2.5),
                c + v(0.0, -6.5),
                c + v(0.0, 6.5),
                c + v(-4.0, 2.5),
                c + v(-7.0, 2.5),
            ]);
            f.stroke(&body, stroke(s, col));
            if matches!(icon, Icon::VolumeMuted) {
                line(f, c + v(3.0, -3.0), c + v(8.0, 3.0), s, col);
                line(f, c + v(8.0, -3.0), c + v(3.0, 3.0), s, col);
            } else {
                for r in [4.0f32, 7.5] {
                    let pts: Vec<Point> = (0..=10)
                        .map(|i| {
                            let a = -0.9 + i as f32 / 10.0 * 1.8;
                            c + v(a.cos(), a.sin()) * r + v(0.5, 0.0)
                        })
                        .collect();
                    polyline(f, &pts, s, col);
                }
            }
        }
        Icon::Fullscreen | Icon::FullscreenExit => {
            let out = matches!(icon, Icon::Fullscreen);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = c + v(sx * 7.0, sy * 7.0);
                let k = if out {
                    corner
                } else {
                    c + v(sx * 3.0, sy * 3.0)
                };
                let (a, b) = if out {
                    (corner - v(sx * 4.0, 0.0), corner - v(0.0, sy * 4.0))
                } else {
                    (k + v(sx * 4.0, 0.0), k + v(0.0, sy * 4.0))
                };
                polyline(f, &[a, k, b], s, col);
            }
        }
        Icon::More => {
            for dy in [-5.5, 0.0, 5.5] {
                f.fill(&Path::circle(c + v(0.0, dy), 1.7), col);
            }
        }
        Icon::External => {
            let s = 1.2;
            polyline(
                f,
                &[
                    c + v(-1.0, -4.0),
                    c + v(-4.0, -4.0),
                    c + v(-4.0, 4.0),
                    c + v(4.0, 4.0),
                    c + v(4.0, 1.0),
                ],
                s,
                col,
            );
            line(f, c + v(-0.5, 0.5), c + v(4.5, -4.5), s, col);
            polyline(
                f,
                &[c + v(1.0, -4.5), c + v(4.5, -4.5), c + v(4.5, -1.0)],
                s,
                col,
            );
        }
        Icon::Home => {
            polyline(
                f,
                &[c + v(-8.0, -0.5), c + v(0.0, -8.0), c + v(8.0, -0.5)],
                1.6,
                col,
            );
            rect(f, c + v(0.0, 2.25), 11.0, 9.5, 1.0, 1.6, col);
        }
        Icon::Film => {
            rect(f, c, 16.0, 13.0, 2.5, 1.6, col);
            f.fill(
                &polygon(&[c + v(-2.5, -3.5), c + v(4.0, 0.0), c + v(-2.5, 3.5)]),
                col,
            );
        }
        Icon::Tv => {
            rect(f, c + v(0.0, -1.5), 17.0, 12.0, 2.0, 1.6, col);
            line(f, c + v(-4.0, 7.5), c + v(4.0, 7.5), 1.6, col);
        }
        Icon::List => {
            for dy in [-5.0, 0.0, 5.0] {
                line(f, c + v(-7.0, dy), c + v(7.0, dy), s, col);
            }
        }
        Icon::SkipBack | Icon::SkipForward => {
            let back = matches!(icon, Icon::SkipBack);
            // an open circle, its gap at the top, with an arrowhead on one end
            let (a0, a1) = (-1.25f32, 4.0f32);
            let pts: Vec<Point> = (0..=28)
                .map(|i| {
                    let a = a0 + i as f32 / 28.0 * (a1 - a0);
                    let a = if back { PI - a } else { a };
                    c + v(a.cos(), a.sin()) * 11.0
                })
                .collect();
            let tip = pts[0];
            polyline(f, &pts, 1.8, col);
            let d = if back { -1.0 } else { 1.0 };
            f.fill(
                &polygon(&[
                    tip + v(-4.0 * d, -4.0),
                    tip + v(3.0 * d, 0.5),
                    tip + v(-4.0 * d, 4.0),
                ]),
                col,
            );
        }
        Icon::Settings => {
            for i in 0..8 {
                let a = i as f32 / 8.0 * TAU;
                let d = v(a.cos(), a.sin());
                line(f, c + d * 5.0, c + d * 8.5, 3.0, col);
            }
            // a ring: the hole shows whatever is behind
            f.stroke(&Path::circle(c, 4.4), stroke(3.6, col));
        }
    }
}

/// A turning arc; `t` is seconds, so it moves as the app redraws.
pub fn spinner<'a, M: 'a>(t: f32, color: Color, size: f32) -> Canvas<Spinner, M> {
    canvas_widget(Spinner { t, color }).width(size).height(size)
}

pub struct Spinner {
    t: f32,
    color: Color,
}

impl<M> canvas::Program<M> for Spinner {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let c = frame.center();
        let r = bounds.width.min(bounds.height) / 2.0 - 2.0;
        let start = self.t * TAU;
        let pts: Vec<Point> = (0..=24)
            .map(|i| {
                let a = start + i as f32 / 24.0 * 1.5 * PI;
                c + v(a.cos(), a.sin()) * r
            })
            .collect();
        polyline(&mut frame, &pts, 3.0, self.color);
        vec![frame.into_geometry()]
    }
}
