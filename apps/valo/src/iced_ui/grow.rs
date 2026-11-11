//! A hovered card or tile drawn a little bigger, in place. Iced's `float`
//! can scale too, but it draws the scaled copy as an overlay clipped to a
//! rectangle that, for a row scrolling sideways inside the scrolling page,
//! ignores how far the page is scrolled: once the page moves down, the copy
//! lands outside its clip and the card vanishes. Cards grow by less than the
//! gap between them, so they can simply be drawn bigger where they are,
//! clipped like everything else in their row.
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{tree, Operation, Tree, Widget};
use iced::advanced::{overlay, Clipboard, Shell};
use iced::{mouse, Element, Event, Length, Rectangle, Size, Transformation, Vector};

pub struct Grow<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    scale: f32,
}

/// `content` drawn `scale` times its size around its centre; it takes the
/// room of its normal size.
pub fn grow<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    scale: f32,
) -> Grow<'a, Message, Theme, Renderer> {
    Grow {
        content: content.into(),
        scale,
    }
}

impl<Message, Theme, Renderer> Grow<'_, Message, Theme, Renderer> {
    fn transformation(&self, bounds: Rectangle) -> Transformation {
        let c = Vector::new(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        );
        Transformation::translate(c.x, c.y)
            * Transformation::scale(self.scale)
            * Transformation::translate(-c.x, -c.y)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Grow<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let inverse = self.transformation(layout.bounds()).inverse();
        self.content.as_widget_mut().update(
            tree,
            event,
            layout,
            cursor * inverse,
            renderer,
            clipboard,
            shell,
            &(*viewport * inverse),
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if self.scale == 1.0 {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
            return;
        }
        let t = self.transformation(layout.bounds());
        let inverse = t.inverse();
        renderer.with_transformation(t, |r| {
            self.content.as_widget().draw(
                tree,
                r,
                theme,
                style,
                layout,
                cursor * inverse,
                &(*viewport * inverse),
            );
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let inverse = self.transformation(layout.bounds()).inverse();
        self.content.as_widget().mouse_interaction(
            tree,
            layout,
            cursor * inverse,
            &(*viewport * inverse),
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<Grow<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(g: Grow<'a, Message, Theme, Renderer>) -> Self {
        Element::new(g)
    }
}
