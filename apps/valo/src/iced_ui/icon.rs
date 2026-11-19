//! The app's icons, from Lucide (lucide.dev, ISC license: assets/lucide/LICENSE),
//! drawn in the theme's colors, and a spinner.
use std::f32::consts::{PI, TAU};

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::svg::{self, Handle, Svg};
use iced::widget::{canvas as canvas_widget, container, Canvas};
use iced::{mouse, Color, Element, Point, Rectangle, Renderer, Theme, Vector};

pub use crate::ui::chrome::Icon;

/// The icon's Lucide SVG, built into the app.
fn source(icon: Icon) -> &'static [u8] {
    match icon {
        Icon::Refresh => include_bytes!("../../assets/lucide/refresh-cw.svg"),
        Icon::Settings => include_bytes!("../../assets/lucide/settings.svg"),
        Icon::Minimize => include_bytes!("../../assets/lucide/minus.svg"),
        Icon::Maximize => include_bytes!("../../assets/lucide/square.svg"),
        Icon::Restore => include_bytes!("../../assets/lucide/copy.svg"),
        Icon::Close => include_bytes!("../../assets/lucide/x.svg"),
        Icon::Bookmark => include_bytes!("../../assets/lucide/bookmark.svg"),
        Icon::BookmarkOn => include_bytes!("../../assets/lucide/bookmark-check.svg"),
        Icon::Play => include_bytes!("../../assets/lucide/play.svg"),
        Icon::ChevronLeft => include_bytes!("../../assets/lucide/chevron-left.svg"),
        Icon::ArrowRight => include_bytes!("../../assets/lucide/arrow-right.svg"),
        Icon::Star => include_bytes!("../../assets/lucide/star.svg"),
        Icon::Pause => include_bytes!("../../assets/lucide/pause.svg"),
        Icon::ChevronRight => include_bytes!("../../assets/lucide/chevron-right.svg"),
        Icon::Subtitles => include_bytes!("../../assets/lucide/captions.svg"),
        Icon::Volume => include_bytes!("../../assets/lucide/volume-2.svg"),
        Icon::VolumeMuted => include_bytes!("../../assets/lucide/volume-x.svg"),
        Icon::Fullscreen => include_bytes!("../../assets/lucide/maximize.svg"),
        Icon::FullscreenExit => include_bytes!("../../assets/lucide/minimize.svg"),
        Icon::More => include_bytes!("../../assets/lucide/ellipsis-vertical.svg"),
        Icon::List => include_bytes!("../../assets/lucide/list.svg"),
        Icon::SkipBack => include_bytes!("../../assets/lucide/rotate-ccw.svg"),
        Icon::SkipForward => include_bytes!("../../assets/lucide/rotate-cw.svg"),
        Icon::ChevronUp => include_bytes!("../../assets/lucide/chevron-up.svg"),
        Icon::ChevronDown => include_bytes!("../../assets/lucide/chevron-down.svg"),
        Icon::ArrowUp => include_bytes!("../../assets/lucide/arrow-up.svg"),
        Icon::ArrowDown => include_bytes!("../../assets/lucide/arrow-down.svg"),
        Icon::External => include_bytes!("../../assets/lucide/external-link.svg"),
        Icon::Home => include_bytes!("../../assets/lucide/house.svg"),
        Icon::Film => include_bytes!("../../assets/lucide/film.svg"),
        Icon::Tv => include_bytes!("../../assets/lucide/tv.svg"),
        Icon::Image => include_bytes!("../../assets/lucide/image.svg"),
        Icon::CloudDownload => include_bytes!("../../assets/lucide/cloud-download.svg"),
    }
}

/// An icon in a `size`-square box, in `color`.
pub fn icon<'a, M: 'a>(icon: Icon, color: Color, size: f32) -> Element<'a, M> {
    icon_hover(icon, color, color, size)
}

/// An icon that turns `hot` while the pointer is over it.
///
/// `size` is the box it sits in; the glyph itself is drawn at most 18px
/// (the skip circles, with the seconds inside them, 26px), centered, so the
/// boxes callers ask for keep the icons the size they always were.
pub fn icon_hover<'a, M: 'a>(icon: Icon, color: Color, hot: Color, size: f32) -> Element<'a, M> {
    let max = if matches!(icon, Icon::SkipBack | Icon::SkipForward) {
        26.0
    } else {
        18.0
    };
    let glyph = size.min(max);
    let svg: Svg<'a> = Svg::new(Handle::from_memory(source(icon)))
        .width(glyph)
        .height(glyph)
        .style(move |_, s| svg::Style {
            color: Some(if matches!(s, svg::Status::Hovered) {
                hot
            } else {
                color
            }),
        });
    container(svg).center(size).into()
}

fn v(x: f32, y: f32) -> Vector {
    Vector::new(x, y)
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
