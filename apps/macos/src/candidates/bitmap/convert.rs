//! 壳的帧类型 → 渲染器的帧类型。两边字段一一对应，spike 定型后壳直接用渲染器的类型，这层就没了。

use crate::candidates::frame::Frame;
use crate::candidates::preedit::{Preedit, PreeditStyle};
use crate::candidates::row::{Row, Tone};

pub(super) fn frame(frame: &Frame) -> qingjian_render::Frame {
    qingjian_render::Frame {
        preedit: frame.preedit.as_ref().map(preedit),
        rows: frame.rows.iter().map(row).collect(),
        highlighted: Some(frame.highlighted),
        columns: frame.columns,
        column_ems: frame.column_ems.clone(),
        footer: frame.footer.clone(),
        sentence: frame.sentence.clone(),
        status: frame.status.clone(),
        mode: frame.mode.clone(),
    }
}

fn preedit(preedit: &Preedit) -> qingjian_render::Preedit {
    qingjian_render::Preedit {
        segments: preedit
            .segments
            .iter()
            .map(|segment| qingjian_render::PreeditSegment {
                text: segment.text.clone(),
                style: match segment.style {
                    PreeditStyle::Typed => qingjian_render::PreeditStyle::Typed,
                    PreeditStyle::Rest => qingjian_render::PreeditStyle::Rest,
                    PreeditStyle::Struck => qingjian_render::PreeditStyle::Struck,
                },
            })
            .collect(),
        cursor: preedit.cursor,
    }
}

fn row(row: &Row) -> qingjian_render::Row {
    qingjian_render::Row {
        index: row.index.clone(),
        text: row.text.clone(),
        code: None,
        annotation: row
            .annotation
            .iter()
            .map(|(text, tone)| {
                let tone = match tone {
                    Tone::Gloss => qingjian_render::Tone::Gloss,
                    Tone::CloudGloss => qingjian_render::Tone::CloudGloss,
                    Tone::Fresh => qingjian_render::Tone::Fresh,
                    Tone::Faint => qingjian_render::Tone::Faint,
                    Tone::Pos => qingjian_render::Tone::Pos,
                    Tone::Separator => qingjian_render::Tone::Separator,
                };
                (text.clone(), tone)
            })
            .collect(),
        cloud: row.cloud,
    }
}

#[cfg(test)]
mod tests {
    use super::row;
    use crate::candidates::row::Row;
    use qingjian_core::{
        Candidate, CandidateKind, Language, Sense, Translation, TranslationSource,
    };
    use qingjian_render::{FontLibrary, Frame, HitTarget, Layout, Preedit, Renderer, Theme};

    #[test]
    fn english_first_frames_keep_both_languages_and_click_regions_in_all_themes() {
        let candidate = Candidate {
            text: "请客".to_owned(),
            kind: CandidateKind::Chinese,
            syllables: vec!["qing".to_owned(), "ke".to_owned()],
            reading: None,
            translation: Some(Translation::new(
                Language::English,
                vec![Sense {
                    part_of_speech: None,
                    text: "treat someone to a meal".to_owned(),
                    reading: None,
                    fresh: false,
                }],
            )),
            aux_code: None,
        };
        let library = FontLibrary::system("zh-CN").unwrap();
        let mut renderer = Renderer::new(library);
        for cloud in [false, true] {
            let mut candidate = candidate.clone();
            if cloud {
                candidate.translation.as_mut().unwrap().source = TranslationSource::Cloud;
            }
            let projected = Row::with_english_first(0, &candidate, true);
            let frame = Frame {
                preedit: Some(Preedit::plain("qing'ke", 7)),
                rows: vec![row(&projected)],
                highlighted: Some(0),
                ..Frame::default()
            };
            assert_eq!(frame.rows[0].text, "treat someone to a meal");
            assert_eq!(frame.rows[0].annotation[0].0, "请客");
            for id in ["qingjian", "system-blue", "wechat", "sakura"] {
                for dark in [false, true] {
                    for layout in [Layout::Vertical, Layout::Horizontal] {
                        let theme = Theme::builtin(id, dark).unwrap();
                        let rendered = renderer.render(&frame, layout, &theme, 2.0).unwrap();
                        for target in [
                            HitTarget::Candidate(0),
                            HitTarget::Translation {
                                candidate: 0,
                                sense: 0,
                            },
                        ] {
                            let region = rendered
                                .hits
                                .iter()
                                .find(|hit| hit.target == target)
                                .unwrap();
                            let (x, y) = (
                                (region.x + region.width / 2.0) / rendered.scale,
                                (region.y + region.height / 2.0) / rendered.scale,
                            );
                            assert_eq!(
                                rendered.hit_at(x, y),
                                Some(target),
                                "{id} {dark} {layout:?}"
                            );
                        }
                        if let Some(dir) = std::env::var_os("QINGJIAN_ENGLISH_FIRST_PREVIEW") {
                            let dir = std::path::PathBuf::from(dir);
                            std::fs::create_dir_all(&dir).unwrap();
                            rendered
                                .pixmap
                                .save_png(dir.join(format!("{id}-{cloud}-{dark}-{layout:?}.png")))
                                .unwrap();
                        }
                    }
                }
            }
        }
    }
}
