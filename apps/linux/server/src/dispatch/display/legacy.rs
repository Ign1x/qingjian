//! 发给 Fcitx5 / Fcitx4 插件的帧：C++ 插件握手报 v3、按 JSON 字段取值，只认 v3 那时就有的写法，也画不了候选旁的码。
//! IBus 前端与 Wayland 前端是 Rust 写的、与 Server 同一次编译，帧里的类型都认得，不走这里。
use qingjian_platform::protocol::Frame;
use serde_json::{Value, json};

/// 插件握手写死的那一版。
pub(super) const PLUGIN_VERSION: u32 = 3;

/// v3 时拼音行分段就有的种类；之后加的（辅码码段 `AuxCode`）当普通拼音段发。
const V3_PREEDIT_KINDS: [&str; 3] = ["Typed", "Rest", "Corrected"];

/// 把帧 `frame` 序列化成的 `value` 改成插件认得的样子：
/// - 拼音行（含窗口里那一行）里 v3 之后才有的分段种类当普通拼音段；
/// - 候选种类必须是字符串：自定义短语的 `{"Custom": n}` 是对象，Fcitx5 插件按字符串取 `kind` 会抛异常、断开整条连接，
///   改报成中文词（插件只认得出 `Cloud`）；
/// - 开着「候选上显示码」时码拼在候选文字后面（`开发[kf]`，与自绘候选窗画的位置一样）：框架面板只有文字与注记两栏，
///   注记归译词。选词按下标回 Server，上屏的仍是原词。
pub(super) fn downgrade(frame: &Frame, value: &mut Value) {
    plain_segments(value.get_mut("preedit"));
    plain_segments(
        value
            .get_mut("window_preedit")
            .and_then(|window| window.get_mut(0)),
    );
    let Some(items) = value
        .get_mut("candidates")
        .and_then(|candidates| candidates.get_mut("items"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for (candidate, item) in frame.candidates.items.iter().zip(items) {
        if !item["kind"].is_string() {
            item["kind"] = json!("Chinese");
        }
        if frame.aux_code_show
            && !candidate.text.is_empty()
            && let Some(code) = &candidate.aux_code
        {
            item["text"] = json!(format!("{}[{code}]", candidate.text));
        }
    }
}

fn plain_segments(segments: Option<&mut Value>) {
    let Some(segments) = segments.and_then(Value::as_array_mut) else {
        return;
    };
    for segment in segments {
        let known = segment
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| V3_PREEDIT_KINDS.contains(&kind));
        if !known {
            segment["kind"] = json!("Typed");
        }
    }
}
