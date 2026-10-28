//! 拼音行：帧里的分段与光标（字符数）；候选窗口里的拼音行另有一串（拼音行显示声调）时用它，应用里的行内拼音不变。
#pragma once
#include <nlohmann/json.hpp>
#include <cstddef>
#include <string>
#include <string_view>

namespace qingjian {
/// 帧里的一串拼音：分段（`[{"text", "kind"}]`）与光标的字符位置。分段引用着帧，帧要活得比它长。
struct PreeditSource {
    const nlohmann::json &segments;

    size_t cursor;
};

/// `text` 里第 `chars` 个字符起始的字节位置（帧里的光标按字符数给，Fcitx 要字节），超出取末尾。
inline size_t byteOffset(std::string_view text, size_t chars) {
    size_t bytes = 0;
    for (size_t seen = 0; bytes < text.size() && seen < chars; ++bytes)
        if ((static_cast<unsigned char>(text[bytes]) & 0xc0) != 0x80) ++seen;
    while (bytes < text.size() && (static_cast<unsigned char>(text[bytes]) & 0xc0) == 0x80) ++bytes;
    return bytes;
}

/// 分段拼成一串。
inline std::string joined(const nlohmann::json &segments) {
    std::string text;
    for (const auto &segment : segments) text += segment.at("text").get<std::string>();
    return text;
}

/// 应用里的行内拼音：帧的 `preedit` 与 `cursor`。
inline PreeditSource inlinePreedit(const nlohmann::json &frame) {
    return {frame.at("preedit"), frame.at("cursor").get<size_t>()};
}

/// 候选窗口里的拼音行：Server 另给了 `window_preedit`（`[分段, 光标]`，带调拼音）就用它，
/// 没有这个字段（老 Server）、是 null 或形状不对时同行内拼音。
inline PreeditSource windowPreedit(const nlohmann::json &frame) {
    const auto found = frame.find("window_preedit");
    if (found == frame.end() || !found->is_array() || found->size() != 2 || !found->at(0).is_array()
        || !found->at(1).is_number_unsigned())
        return inlinePreedit(frame);
    return {found->at(0), found->at(1).get<size_t>()};
}
}
