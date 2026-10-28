//! 拼音行的光标换算与窗口那一串的取法：带调拼音另给时用它，老 Server、null 与形状不对的退回行内拼音。
#include "../preedit.h"
#include <cassert>

int main() {
    assert(qingjian::byteOffset("ni'hao", 6) == 6);
    assert(qingjian::byteOffset("nǐ'hǎo", 2) == 3);
    assert(qingjian::byteOffset("nǐ'hǎo", 6) == 8);
    assert(qingjian::byteOffset("nǐ'hǎo", 99) == 8);
    assert(qingjian::byteOffset("你好shi", 2) == 6);
    assert(qingjian::byteOffset("", 3) == 0);

    auto frame = nlohmann::json::parse(R"({"preedit":[{"text":"你好","kind":"Typed"},{"text":"shi'jie","kind":"Typed"}],"cursor":5})");
    const auto plain = qingjian::inlinePreedit(frame);
    assert(qingjian::joined(plain.segments) == "你好shi'jie" && plain.cursor == 5);
    assert(&qingjian::windowPreedit(frame).segments == &frame.at("preedit"));
    for (const auto *other : {"null", "[]", R"([[{"text":"x"}], -1])", R"(["x", 1])"}) {
        frame["window_preedit"] = nlohmann::json::parse(other);
        const auto window = qingjian::windowPreedit(frame);
        assert(&window.segments == &frame.at("preedit") && window.cursor == 5);
    }

    frame["window_preedit"] = nlohmann::json::parse(R"([[{"text":"你好","kind":"Typed"},{"text":"shì'jiè","kind":"Typed"}],9])");
    const auto toned = qingjian::windowPreedit(frame);
    assert(qingjian::joined(toned.segments) == "你好shì'jiè" && toned.cursor == 9);
    assert(qingjian::byteOffset(qingjian::joined(toned.segments), toned.cursor) == 15);
    assert(qingjian::joined(qingjian::inlinePreedit(frame).segments) == "你好shi'jie");
}
