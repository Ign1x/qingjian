//! 真 Server 开着拼音行显示声调：Fcitx5 面板上的拼音行带调，应用里的行内拼音不变，两边的光标各按各的字节算。
#include "qingjian.h"
#include "support/context.h"
#include "support/server.h"
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputpanel.h>

int main(int argc, char **argv) {
    assert(argc == 2);
    const std::string mode = argv[1];
    Server server("show_tones = true\npreedit = \"" + mode + "\"\n");
    char program[] = "qingjian-test"; char disable[] = "--disable=all";
    char *arguments[] = {program, disable, nullptr};
    fcitx::Instance instance(2, arguments);
    instance.initialize();
    fcitx::QingjianEngine engine(&instance.addonManager());
    Context context(instance.inputContextManager());
    context.setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
    context.focusIn();
    for (char c : std::string("nihao")) assert(engine.process(&context, fcitx::Key(static_cast<fcitx::KeySym>(c))));
    const auto &panel = context.inputPanel();
    assert(panel.candidateList() && panel.candidateList()->candidate(0).text().toString() == "你好");
    if (mode == "window") assert(panel.clientPreedit().empty());
    else assert(panel.clientPreedit().toString() == "ni'hao" && panel.clientPreedit().cursor() == 6);
    if (mode == "inline") assert(panel.preedit().empty());
    else assert(panel.preedit().toString() == "nǐ'hǎo" && panel.preedit().cursor() == 8);
}
