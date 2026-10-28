---
title: Platform differences
order: 1
description: "Which Qingjian features are available on macOS, Windows, Linux and Android: current status, key conventions and what isn't supported yet."
---

The macOS and Windows versions are in beta with nearly the same features; keys follow each system's conventions. The Linux version comes in two editions, both installed from a terminal: the fcitx edition uses the Fcitx / IBus candidate panel, and the Wayland native edition draws its own candidate window. The Linux column in the tables below is the fcitx edition; for how the Wayland native edition differs, see [The two Linux editions](#The-two-Linux-editions). The Android version is a preview with fewer features, installed from an APK.

"✓" means supported; "—" means not supported yet, or not applicable.

## Typing

| Feature | macOS | Windows | Linux | Android |
|---|---|---|---|---|
| Full pinyin, sentences, abbreviated pinyin, spelling correction | ✓ | ✓ | ✓ | ✓ |
| 9-key keypad | — | — | — | ✓ |
| Double pinyin, fuzzy pinyin | ✓ | ✓ | ✓ | ✓ |
| Dachen Zhuyin | — | ✓ | ✓ | ✓ |
| Wubi, Wubi–pinyin mixed input | ✓ | ✓ | ✓ | ✓ |
| Custom double pinyin / shape-based schemes | ✓ | ✓ | ✓ | — |
| Auxiliary code | ✓ | ✓ | ✓ | — |
| Key hints and radical chart | ✓ | ✓ | — | — |
| Local sentence model | ✓ | ✓ | ✓ | — |
| English mode and English completion | ✓ | ✓ | ✓ | ✓ (English keyboard) |
| Choice of Chinese/English switch key | — (`Caps Lock`) | ✓ | ✓ | — |
| Start in English mode | — | ✓ | ✓ | — |
| Remember mode per app | — | ✓ | ✓ | — |
| Automatic spacing between Chinese and Latin text | ✓ | ✓ | ✓ | — |
| Symbol panel | ✓ | ✓ | — | ✓ (symbol keyboard) |
| Dates, calculations, characters by code point | ✓ | ✓ | ✓ | ✓ |
| Traditional Chinese output | ✓ | ✓ | ✓ | ✓ |
| Custom phrases | ✓ | ✓ | ✓ | ✓ (saved phrases) |
| System text replacements | ✓ | — | — | — |
| Importing pinyin dictionaries | ✓ | ✓ | ✓ | ✓ |
| Importing Wubi dictionaries | ✓ | ✓ | ✓ | — |
| Voice input, swipe typing, clipboard | — | — | — | ✓ |

## Learning languages

| Feature | macOS | Windows | Linux | Android |
|---|---|---|---|---|
| Glosses beside candidates and new-word marks | ✓ | ✓ | ✓ (first gloss only) | ✓ |
| Committing glosses directly | ✓ | ✓ | ✓ | ✓ (long-press a candidate) |
| Vocabulary book, statistics | ✓ | ✓ | ✓ | ✓ |

## Appearance

| Feature | macOS | Windows | Linux | Android |
|---|---|---|---|---|
| Seven built-in themes, light and dark | ✓ | ✓ | — (Fcitx panel) | ✓ |
| Custom themes | ✓ | ✓ | — | — |
| Expanding horizontal layout into rows | ✓ | ✓ | — | — |
| Tones on the pinyin line | ✓ | ✓ | ✓ | — |
| Floating status bar | — | ✓ | — | — |

## Keys and shortcuts

| Feature | macOS | Windows | Linux | Android |
|---|---|---|---|---|
| Changing paging keys | ✓ | ✓ | ✓ | — |
| Extra paging keys, highlight keys | ✓ | ✓ | ✓ | — |
| Full-width/half-width punctuation shortcut | ✓ | ✓ | ✓ | — |

## AI services

| Feature | macOS | Windows | Linux | Android |
|---|---|---|---|---|
| Cloud suggestions and sentence completion | ✓ | ✓ | ✓ | ✓ |
| On-demand suggestions, request timing | ✓ | ✓ | ✓ | — |
| Asking about a character | ✓ | ✓ | ✓ | — |
| Translating selected text | ✓ | ✓ | ✓ | ✓ |
| Quick translation bar | — | — | — | ✓ |
| Filling in glosses automatically | ✓ | ✓ | ✓ | ✓ |
| Sentence translation for the vocabulary book | ✓ | ✓ | ✓ | — |
| Translating sentence candidates | — | ✓ | — | — |
| AI usage statistics | ✓ | ✓ | ✓ | — |

Installation and platform-specific features: [Installation](../getting-started/install.md) (macOS, Windows), [Linux](linux.md), [Android](android.md).

## The two Linux editions

The Linux column in the tables above is the fcitx edition. The Wayland native edition is for desktops such as Hyprland, sway and niri, and differs from the fcitx edition only in the items below; everything else is the same. For installing and setting it up, see [Linux](linux.md#Wayland-native-edition).

| Feature | fcitx edition | Wayland native edition |
|---|---|---|
| Candidate window | Fcitx / IBus candidate panel | Drawn by Qingjian, same as macOS and Windows |
| Seven built-in themes, light and dark | — | ✓ (can follow the desktop's light/dark preference) |
| Custom themes, candidate fonts and sizes | — | ✓ |
| Glosses beside candidates | First gloss only | All glosses |
| Key hints (double pinyin keyboard, Wubi radical chart) | — | ✓ |
| Expanding a horizontal layout into rows | — | ✓ (with the up and down arrow keys) |
| Mode-switch hint | Shown by Fcitx5 | Briefly shows 中 or 英 where the candidate window appears, with the scheme name for Shuangpin and other schemes |
| Tray | Provided by Fcitx / IBus | Qingjian's 中 / 英 icon; left-click to switch, right-click for mode, punctuation, traditional output or settings (requires status bar support) |
| Candidates in English mode | Per the app list | Per the app list; if the desktop can't identify apps, untick 只在写字的应用里给 (Only in writing apps) |
| Candidate right-click menu (pin, demote, delete) | — | ✓ |
| X11 apps | ✓ | — |
