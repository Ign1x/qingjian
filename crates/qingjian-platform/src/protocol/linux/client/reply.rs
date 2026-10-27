//! Server 对一次事件的答复。

use serde_json::Value;

use super::ClientError;
use crate::protocol::linux::DisplayIdentity;
use crate::protocol::{Frame, KeyOutcome, ServerMessage};

/// 一次事件之后 Server 让前端做的事。
///
/// 三样都由 Server 定：**前端不判断这个键该不该吃、该上什么屏**
/// （`docs/contributing.md`「架构约束」）。
#[derive(Debug)]
pub struct Reply {
    /// 这次按键吃掉还是放行给应用。
    pub outcome: KeyOutcome,

    /// 立刻上屏的文本；没有就是 `None`。
    pub commit: Option<String>,

    /// 要画的组句状态；空帧表示收起候选窗口。
    pub frame: Frame,

    /// 这一帧的展示身份：Linux Server 附在回包上的 `identity`，没有就是 `None`。
    /// 组句期间的轮询靠里面的 `revision` 判断帧换没换（重排结果到了才换），版本号没变就不重画；
    /// 点选、翻页与展示回执要原样带回去，Server 据此丢掉过期的那些。
    pub identity: Option<DisplayIdentity>,

    /// 这次按键切了中 / 英：`Some(true)` 切到英文。只在切换的那一下有，前端拿它弹提示。
    pub english: Option<bool>,

    /// 选区翻译还在等结果：preedit 与候选都空着也得接着轮询（Linux Server 在帧上另附的字段）。
    pub translation_pending: bool,
}

impl Default for Reply {
    /// 缺省是**放行**：认不出来的回包不能把用户的键吞掉，宁可让应用自己收到。
    fn default() -> Self {
        Self {
            outcome: KeyOutcome::Passthrough,
            commit: None,
            frame: Frame::default(),
            identity: None,
            english: None,
            translation_pending: false,
        }
    }
}

impl Reply {
    /// 从 Server 的一条回包里读出来。
    ///
    /// 正常是 `KeyResult`，轮询回 `Update`；会话已经不在了、或点选的那一帧已经过期，Server 回 `Ignored`（当作没事发生）。
    pub fn from_value(value: Value) -> Result<Self, ClientError> {
        if value.get("Ignored").is_some() {
            return Ok(Self::default());
        }
        // 展示身份与 translation_pending 不在共用的消息类型里，是 Linux Server 在回包上另附的，按原始 JSON 取
        let body = value
            .as_object()
            .and_then(|message| message.values().next());
        let identity = body
            .and_then(|body| body.get("identity"))
            .and_then(|identity| serde_json::from_value(identity.clone()).ok());
        let translation_pending = body
            .and_then(|body| body.get("frame"))
            .and_then(|frame| frame.get("translation_pending"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        match serde_json::from_value::<ServerMessage>(value.clone()) {
            Ok(ServerMessage::KeyResult {
                outcome,
                commit,
                frame,
                english,
                ..
            }) => Ok(Self {
                outcome,
                commit,
                frame,
                identity,
                english,
                translation_pending,
            }),
            Ok(ServerMessage::Update { frame, commit, .. }) => Ok(Self {
                commit,
                frame,
                identity,
                translation_pending,
                ..Self::default()
            }),
            _ => Err(ClientError::Unexpected(value.to_string())),
        }
    }

    /// 这一帧的版本号；回包上没带展示身份时是 `None`。
    pub fn revision(&self) -> Option<u64> {
        self.identity.as_ref().map(|identity| identity.revision)
    }
}
