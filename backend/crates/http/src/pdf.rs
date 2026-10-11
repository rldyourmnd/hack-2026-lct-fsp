//! `exportMyProfile`: a minimal PDF writer on top of `krilla`. The DejaVu Sans
//! font is bundled so the file renders Cyrillic text on any viewer.

use fsp_application::ProfileView;
use krilla::Data;
use krilla::Document;
use krilla::color::rgb;
use krilla::geom::Point;
use krilla::page::PageSettings;
use krilla::paint::Fill;
use krilla::text::{Font, TextDirection};

const FONT_BYTES: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");
const FONT_SIZE_NAME: f32 = 18.0;
const FONT_SIZE_LABEL: f32 = 10.5;
const FONT_SIZE_TEXT: f32 = 11.5;
const FONT_SIZE_FOOT: f32 = 9.0;
const MARGIN: f32 = 50.0;
const BOTTOM: f32 = 842.0 - MARGIN;
const WIDTH: f32 = 595.0 - 2.0 * MARGIN;

fn a4() -> PageSettings {
    PageSettings::from_wh(595.0, 842.0).expect("A4 page settings")
}

fn ink(color: rgb::Color) -> Fill {
    Fill {
        paint: color.into(),
        ..Default::default()
    }
}

fn heading_color() -> rgb::Color {
    rgb::Color::new(26, 46, 86)
}
fn label_color() -> rgb::Color {
    rgb::Color::new(96, 106, 120)
}
fn text_color() -> rgb::Color {
    rgb::Color::new(23, 26, 33)
}
fn foot_color() -> rgb::Color {
    rgb::Color::new(120, 126, 140)
}

/// Rough word-wrap: the bundled font is the only face in play, so an
/// average glyph width estimate is enough for a demo document.
fn wrap(text: &str, size: f32) -> Vec<String> {
    let limit = (WIDTH / (size * 0.52)).max(16.0) as usize;
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if !current.is_empty() && next_len > limit {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

type Line = (f32, u8, String);
const HEADING: u8 = 0;
const LABEL: u8 = 1;
const TEXT: u8 = 2;
const FOOT: u8 = 3;

/// Splits wrapped lines into pages that fit under `BOTTOM`.
fn paginate(lines: &[Line]) -> Vec<Vec<(f32, u8, String, f32)>> {
    let mut pages = Vec::new();
    let mut page: Vec<(f32, u8, String, f32)> = Vec::new();
    let mut y = MARGIN;
    for &(size, kind, ref text) in lines {
        for chunk in wrap(text, size) {
            let leading = size * 1.35;
            if y + leading > BOTTOM && !page.is_empty() {
                pages.push(std::mem::take(&mut page));
                y = MARGIN;
            }
            page.push((size, kind, chunk, y));
            y += leading;
        }
    }
    pages.push(page);
    pages
}

fn grade_label(grade: Option<fsp_domain::Grade>) -> String {
    grade.map_or("—", fsp_domain::Grade::label).to_string()
}

fn specialization_label(spec: fsp_domain::Specialization) -> &'static str {
    spec.short_label()
}

pub fn profile_pdf(profile: &ProfileView) -> Result<Vec<u8>, String> {
    let font = Font::new(Data::from(FONT_BYTES.to_vec()), 0)
        .ok_or_else(|| "embedded font is invalid".to_string())?;

    let mut lines: Vec<Line> = Vec::new();
    let mut push = |size: f32, kind: u8, label: &str, value: String| {
        if !label.is_empty() {
            lines.push((FONT_SIZE_LABEL, LABEL, label.to_string()));
        }
        lines.push((size, kind, value));
        lines.push((FONT_SIZE_FOOT, TEXT, String::new()));
    };

    push(FONT_SIZE_NAME, HEADING, "", profile.name.clone());
    push(FONT_SIZE_TEXT, TEXT, "E-mail", profile.email.clone());
    push(
        FONT_SIZE_TEXT,
        TEXT,
        "Специализация",
        specialization_label(profile.specialization).to_string(),
    );
    push(
        FONT_SIZE_TEXT,
        TEXT,
        "Подтверждённый грейд",
        grade_label(profile.verified_grade),
    );
    push(
        FONT_SIZE_TEXT,
        TEXT,
        "Выбранный грейд",
        grade_label(Some(profile.selected_grade)),
    );
    if !profile.stack.is_empty() {
        push(FONT_SIZE_TEXT, TEXT, "Технологии", profile.stack.join(", "));
    }
    if !profile.team_role.is_empty() {
        push(
            FONT_SIZE_TEXT,
            TEXT,
            "Роль в команде",
            profile.team_role.clone(),
        );
    }
    let years = |value: u32| match (value % 10, value % 100) {
        (_, 11..=14) => "лет",
        (1, _) => "год",
        (2..=4, _) => "года",
        _ => "лет",
    };
    let tenure = match (profile.experience_since, profile.experience_months) {
        (Some(since), months) => {
            let count = months.map_or(0, |months| months / 12);
            Some(format!("с {since} года — {count} {}", years(count)))
        }
        (None, Some(months)) if months >= 12 => Some(format!(
            "{} {}{}",
            months / 12,
            years(months / 12),
            if months % 12 > 0 {
                format!(" {} мес.", months % 12)
            } else {
                String::new()
            }
        )),
        (None, Some(months)) => Some(format!("{months} мес.")),
        (None, None) => None,
    };
    if let Some(tenure) = tenure {
        push(FONT_SIZE_TEXT, TEXT, "В IT", tenure);
    }
    if !profile.experience.is_empty() {
        push(FONT_SIZE_TEXT, TEXT, "Опыт", profile.experience.clone());
    }
    if !profile.soft_skills.is_empty() {
        push(
            FONT_SIZE_TEXT,
            TEXT,
            "Гибкие навыки",
            profile.soft_skills.join(", "),
        );
    }
    push(
        FONT_SIZE_FOOT,
        FOOT,
        "",
        "Сгенерировано демонстрационным бэкендом ФСП. Документ носит \
         ознакомительный характер."
            .to_string(),
    );

    let pages = paginate(&lines);
    let mut document = Document::new();
    for page_lines in pages {
        let mut page = document.start_page_with(a4());
        let mut surface = page.surface();
        for (size, kind, text, y) in page_lines {
            let color = match kind {
                HEADING => heading_color(),
                LABEL => label_color(),
                FOOT => foot_color(),
                _ => text_color(),
            };
            surface.set_fill(Some(ink(color)));
            surface.draw_text(
                Point::from_xy(MARGIN, y),
                font.clone(),
                size,
                &text,
                false,
                TextDirection::Auto,
            );
        }
        surface.finish();
        page.finish();
    }
    document.finish().map_err(|error| error.to_string())
}
