//! 方案与辅码热加载后重查当前组句，挂起会话在恢复焦点时再查，旧帧不能继续用于选词。

use crate::dispatch::Router;

impl Router {
    pub(in crate::dispatch) fn refresh_input(&mut self, reset_aux: bool) {
        if reset_aux {
            self.engine.clear_aux();
        }
        if !self.engine.composition().is_empty() {
            self.recompose(None);
        }
        for (session, info) in &mut self.sessions {
            if Some(*session) != self.focused {
                if reset_aux {
                    info.engine.clear_aux();
                }
                info.needs_refresh = true;
            }
            if let Some(identity) = &mut info.display_identity {
                self.display_revision += 1;
                identity.revision = self.display_revision;
            }
            info.display_frame = None;
            info.last_frame = None;
            info.last_key_hint = None;
        }
        self.engine.note_displayed(std::iter::empty());
    }
}
