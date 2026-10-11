//! Writes the golden estimates the development mock serves (`contracts/fixtures/profile-estimates.json`).
//!
//! The mock connects the same GitHub fixture report and the same fictional FSP result, so it applies
//! these estimates instead of reimplementing the rules. Regenerate with `FSP_UPDATE_FIXTURES=1`.

use std::path::PathBuf;

use fsp_domain::{EstimateInput, FspResult, GitHubReport, estimate_profile};

/// The mock's published result of `FSP-DEMO-024` (`mock-server/src/fsp-fixture.ts`).
fn fsp_result() -> FspResult {
    FspResult {
        competition_id: "competition-demo-api".into(),
        competition_name: "Учебный кубок API 2026 (вымышленное событие)".into(),
        discipline: "Продуктовое программирование".into(),
        starts_at: "2026-09-20T09:00:00Z".into(),
        ends_at: "2026-09-21T18:00:00Z".into(),
        scope: "team".into(),
        team_name: Some("Команда Пример".into()),
        team_size: Some(4),
        status: "published".into(),
        place: Some(3),
        field_size: 18,
        score: Some(82),
        protocol_reference: Some("DEMO-PROTOCOL-API-2026-01".into()),
        published_at: Some("2026-09-22T12:00:00Z".into()),
        synthetic: true,
    }
}

#[test]
fn golden_estimates_match_the_rules() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures");
    let report: GitHubReport = serde_json::from_str(
        &std::fs::read_to_string(fixtures.join("github-report.json")).unwrap(),
    )
    .unwrap();
    let fsp = [fsp_result()];
    let estimate = |github: Option<&GitHubReport>, fsp: &[FspResult]| {
        estimate_profile(&EstimateInput {
            github,
            fsp,
            stated_experience_months: None,
            stated_team_role: None,
            now: "2026-01-01T00:00:00Z",
        })
    };
    let value = serde_json::json!({
        "none": estimate(None, &[]),
        "github": estimate(Some(&report), &[]),
        "fsp": estimate(None, &fsp),
        "githubFsp": estimate(Some(&report), &fsp),
    });
    let path = fixtures.join("profile-estimates.json");
    let rendered = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
    if std::env::var_os("FSP_UPDATE_FIXTURES").is_some() {
        std::fs::write(&path, rendered).unwrap();
        return;
    }
    let stored = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        stored, rendered,
        "profile-estimates.json is stale; regenerate with FSP_UPDATE_FIXTURES=1"
    );
}
