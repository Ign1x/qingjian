//! 一条连接上开着的会话，一个应用一个；满了关最久没用的。

use qingjian_platform::protocol::SessionId;

use super::opened::Opened;

/// 同时开着的会话最多几个。再多就关掉最久没用的那个，那个应用的中 / 英状态随之回到缺省。
const MAX: usize = 16;

#[derive(Debug, Default)]
pub struct Sessions {
    opened: Vec<Opened>,

    /// 用了多少次：给最近用过的排先后。
    clock: u64,
}

impl Sessions {
    pub fn get(&self, app: Option<&str>) -> Option<&Opened> {
        self.opened
            .iter()
            .find(|opened| opened.app.as_deref() == app)
    }

    pub fn get_mut(&mut self, app: Option<&str>) -> Option<&mut Opened> {
        self.opened
            .iter_mut()
            .find(|opened| opened.app.as_deref() == app)
    }

    /// 要在这个应用的会话上发请求了：记一次「刚用过」，交出会话编号；还没开是 `None`。
    pub fn touch(&mut self, app: Option<&str>) -> Option<SessionId> {
        self.clock += 1;
        let clock = self.clock;
        let opened = self.get_mut(app)?;
        opened.used = clock;
        Some(opened.id)
    }

    /// 开新会话之前腾位置，交出要关掉的那个会话。同一条连接上上下文不能重复（Server 直接断开），
    /// 两个应用的哈希撞了就先关旧的那个；满了就关最久没用的。
    pub fn evict(&mut self, context: &str) -> Option<SessionId> {
        let index = self
            .opened
            .iter()
            .position(|opened| opened.context == context)
            .or_else(|| {
                (self.opened.len() >= MAX)
                    .then(|| {
                        self.opened
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, opened)| opened.used)
                            .map(|(index, _)| index)
                    })
                    .flatten()
            })?;
        Some(self.opened.swap_remove(index).id)
    }

    /// 记下刚开的会话，算刚用过。
    pub fn add(&mut self, mut opened: Opened) {
        self.clock += 1;
        opened.used = self.clock;
        self.opened.push(opened);
    }

    /// 连接断了：会话编号只在那条连接上有意义。
    pub fn clear(&mut self) {
        self.opened.clear();
    }
}

#[cfg(test)]
mod tests {
    use qingjian_platform::protocol::linux::Capabilities;

    use super::*;

    fn opened(app: &str, id: u64) -> Opened {
        Opened {
            app: Some(app.to_owned()),
            context: format!("context-{app}"),
            id: SessionId(id),
            capabilities: Capabilities::default(),
            used: 0,
        }
    }

    /// 没满不关；满了关最久没用的，用过的往后排。
    #[test]
    fn the_least_recently_used_goes_first() {
        let mut sessions = Sessions::default();
        for id in 0..MAX as u64 - 1 {
            sessions.add(opened(&format!("app{id}"), id));
        }
        assert_eq!(sessions.evict("context-new"), None, "还有空位");
        sessions.add(opened("last", 99));

        assert_eq!(sessions.touch(Some("app0")), Some(SessionId(0)));
        assert_eq!(sessions.evict("context-new"), Some(SessionId(1)));
        assert!(sessions.get(Some("app1")).is_none());
        assert_eq!(sessions.touch(Some("app1")), None, "关掉的要重开");
    }

    /// 两个应用的上下文撞了：没满也先关旧的那个。
    #[test]
    fn a_colliding_context_is_closed_first() {
        let mut sessions = Sessions::default();
        sessions.add(opened("a", 1));
        sessions.add(opened("b", 2));
        assert_eq!(sessions.evict("context-b"), Some(SessionId(2)));
        assert!(sessions.get(Some("b")).is_none());
        assert!(sessions.get(Some("a")).is_some());
    }

    /// 不知道是哪个应用的那一个会话与有名字的应用分开记。
    #[test]
    fn the_shared_session_is_its_own_entry() {
        let mut sessions = Sessions::default();
        sessions.add(Opened {
            app: None,
            ..opened("shared", 1)
        });
        sessions.add(opened("foot", 2));
        assert_eq!(sessions.touch(None), Some(SessionId(1)));
        assert_eq!(sessions.touch(Some("foot")), Some(SessionId(2)));
    }
}
