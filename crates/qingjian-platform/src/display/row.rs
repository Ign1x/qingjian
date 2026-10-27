//! 候选窗口的一行：[`Candidate`] → 渲染器的 [`Row`]（序号、候选词、注记片段）。

use qingjian_core::{Candidate, CandidateKind};
use qingjian_render::{Row, Tone};

/// `position` 是页内下标（从 0 起）。`show_code` 是 `[general] aux_code_show`：
/// 打开且候选带码时，码用方括号括起来紧跟在候选词后面（`鹤[rbm]`），不进注记。
/// `show_pos` 是 `[general] translation_pos`：关掉时译词保留、词性段（v. / n. / adj.）不画。
pub fn from_candidate(
    position: usize,
    candidate: &Candidate,
    show_code: bool,
    show_pos: bool,
) -> Row {
    let code = candidate
        .aux_code
        .as_ref()
        .filter(|_| show_code)
        .map(|code| format!("[{code}]"));
    let mut annotation = Vec::new();
    if let Some(reading) = &candidate.reading {
        annotation.push((reading.clone(), Tone::Gloss));
    }
    if let Some(translation) = &candidate.translation {
        for (i, sense) in translation.senses().iter().enumerate() {
            if i > 0 || !annotation.is_empty() {
                annotation.push((" · ".to_owned(), Tone::Separator));
            }
            if show_pos && let Some(pos) = sense.part_of_speech {
                annotation.push((format!("{pos} "), Tone::Pos));
            }
            let tone = if sense.fresh {
                Tone::Fresh
            } else {
                Tone::Gloss
            };
            for segment in sense.furigana() {
                annotation.push((segment.text, tone));
                if let Some(reading) = segment.reading {
                    annotation.push((format!("({reading})"), Tone::Faint));
                }
            }
        }
    }
    Row {
        index: (position + 1).to_string(),
        text: candidate.text.clone(),
        code,
        annotation,
        cloud: candidate.kind == CandidateKind::Cloud,
    }
}

#[cfg(test)]
mod tests {
    use qingjian_core::{Language, PartOfSpeech, Sense, Translation};

    use super::*;

    fn candidate_with_gloss() -> Candidate {
        Candidate {
            text: "开发".to_owned(),
            kind: CandidateKind::Chinese,
            syllables: vec!["kai".to_owned(), "fa".to_owned()],
            reading: None,
            translation: Some(Translation::new(
                Language::English,
                vec![Sense {
                    part_of_speech: Some(PartOfSpeech::Verb),
                    text: "develop".to_owned(),
                    reading: None,
                    fresh: false,
                }],
            )),
            aux_code: None,
        }
    }

    fn joined(row: &Row) -> String {
        row.annotation
            .iter()
            .map(|(text, _)| text.as_str())
            .collect()
    }

    #[test]
    fn part_of_speech_follows_the_frame_switch() {
        let on = from_candidate(0, &candidate_with_gloss(), false, true);
        assert!(joined(&on).contains("v. "), "{}", joined(&on));
        assert!(joined(&on).contains("develop"), "{}", joined(&on));

        // 关掉后词性段消失，译词与分隔结构原样保留
        let off = from_candidate(0, &candidate_with_gloss(), false, false);
        assert!(!joined(&off).contains("v. "), "{}", joined(&off));
        assert!(joined(&off).contains("develop"), "{}", joined(&off));
        assert_eq!(on.annotation.len() - 1, off.annotation.len());
    }

    #[test]
    fn a_gloss_without_part_of_speech_is_unchanged_by_the_switch() {
        let mut candidate = candidate_with_gloss();
        candidate.translation.as_mut().unwrap().senses_mut()[0].part_of_speech = None;
        for show_pos in [true, false] {
            assert_eq!(
                joined(&from_candidate(0, &candidate, false, show_pos)),
                "develop"
            );
        }
    }

    /// 全部义项都画（fcitx 版只画第一条），之间用分隔符；生词用强调色；云端候选带云标。
    #[test]
    fn every_sense_is_drawn_and_fresh_ones_stand_out() {
        let mut candidate = candidate_with_gloss();
        candidate.kind = CandidateKind::Cloud;
        let develop = candidate.translation.as_ref().unwrap().senses()[0].clone();
        let exploit = Sense {
            part_of_speech: None,
            text: "exploit".to_owned(),
            reading: None,
            fresh: true,
        };
        candidate.translation = Some(Translation::new(Language::English, vec![develop, exploit]));
        let row = from_candidate(2, &candidate, false, true);
        assert_eq!(row.index, "3");
        assert!(row.cloud);
        assert_eq!(joined(&row), "v. develop · exploit");
        assert!(
            row.annotation
                .contains(&(" · ".to_owned(), Tone::Separator))
        );
        assert!(
            row.annotation
                .contains(&("exploit".to_owned(), Tone::Fresh))
        );
    }

    /// 辅码开关打开时码跟在候选词后面，不进注记；关着不画。
    #[test]
    fn aux_code_is_shown_only_when_switched_on() {
        let mut candidate = candidate_with_gloss();
        candidate.aux_code = Some("kf".to_owned());
        assert_eq!(
            from_candidate(0, &candidate, true, true).code.as_deref(),
            Some("[kf]")
        );
        assert_eq!(from_candidate(0, &candidate, false, true).code, None);
    }
}
