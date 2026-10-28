//! 候选右键菜单在 popup 上：与候选窗合成进同一张位图（input-method-v2 只有一个 popup，位置由合成器定），
//! 卡片左上角摆在右键按下的地方，popup 跟着变大；鼠标在菜单上移动换悬停项重画。弹不弹、点了哪项由会话定。

use crate::session::Spot;
use crate::view::Menu;
use crate::wayland::state::Wayland;

impl Wayland {
    /// 在最近一次按下鼠标的地方弹出菜单；画不出来（候选窗没画出来、菜单的字体库建不起来）返回 `false`。
    pub(super) fn open_menu(&mut self, items: &[String]) -> bool {
        let pressed_at = self.pointer.as_ref().and_then(|pointer| pointer.pressed_at);
        let (Some(canvas), Some((x, y))) = (&self.canvas, pressed_at) else {
            return false;
        };
        let anchor = canvas.anchor(x, y);
        let scale = self.popup.scale() as f32;
        let Some(rendered) = self
            .painter
            .as_mut()
            .and_then(|painter| painter.render_menu(items, None, scale))
        else {
            return false;
        };
        self.drop_clicks();
        self.menu = Some(Menu::new(items.to_vec(), anchor, rendered));
        self.paint();
        // 弹出来时鼠标多半就停在第一项上
        self.hover();
        true
    }

    /// 收起菜单，候选窗照旧。
    pub(super) fn close_menu(&mut self) {
        if self.menu.take().is_some() {
            self.drop_clicks();
            self.paint();
        }
    }

    /// 指针动了（进出候选窗、移动）：菜单开着时换悬停项，换了就重画。
    pub(super) fn hover(&mut self) {
        let position = self.pointer.as_ref().and_then(|pointer| pointer.position);
        let hovered = position
            .zip(self.canvas.as_ref())
            .and_then(|((x, y), canvas)| match canvas.spot(x, y) {
                Some(Spot::MenuItem(item)) => Some(item),
                _ => None,
            });
        let scale = self.popup.scale() as f32;
        let (Some(menu), Some(painter)) = (&mut self.menu, &mut self.painter) else {
            return;
        };
        if menu.hovered == hovered {
            return;
        }
        menu.hovered = hovered;
        if let Some(rendered) = painter.render_menu(menu.items(), hovered, scale) {
            menu.rendered = rendered;
            self.paint();
        }
    }
}
