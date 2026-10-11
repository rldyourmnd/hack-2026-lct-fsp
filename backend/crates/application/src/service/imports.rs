use std::collections::BTreeSet;

use fsp_domain::{ImportEvidence, PdfPage, ResumeExtraction};
use serde_json::{Value, json};

use super::*;

impl PlatformService {
    pub async fn import_profile(
        &self,
        session: &SessionView,
        bytes: &[u8],
    ) -> Result<JobView, ApplicationError> {
        let id = candidate_id(session)?;
        self.require_processing_consent(id).await?;
        let profile = self
            .repository
            .candidate(id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Профиль не найден"))?;
        let (Some(pdf), Some(llm)) = (&self.pdf, &self.llm) else {
            return Err(ApplicationError::new(
                503,
                "pdf_import_unavailable",
                "Импорт PDF не настроен. Заполните профиль вручную",
            ));
        };
        // Both integrations finish before acquiring any database lock.
        let pages = pdf.extract(bytes).await.map_err(|error| {
            let status = match error.code {
                "pdf_parser_unavailable" | "pdf_parser_busy" => 503,
                _ => 422,
            };
            ApplicationError::new(status, error.code, error.message)
        })?;
        let extraction = llm
            .extract_resume(&pages)
            .await
            .map_err(integration_error)?;
        let (preview, evidence) = validated_preview(&profile, extraction, &pages)?;
        let tx = self
            .repository
            .transaction()
            .await
            .map_err(ApplicationError::storage)?;
        tx.lock_candidate(id)
            .await
            .map_err(ApplicationError::storage)?;
        let current = tx
            .candidate(id)
            .await
            .map_err(ApplicationError::storage)?
            .ok_or_else(|| ApplicationError::new(404, "not_found", "Профиль не найден"))?;
        require_current_profile(&current, profile.revision)?;
        let mut job = self.new_job(id, "profile_import", None, Some(profile.revision));
        job.status = "succeeded".into();
        job.updated_at = now();
        job.result_json = Some(self.job_result(&JobResultView::ProfileImport {
            preview,
            parser: "poppler_bifrost",
            simulation: false,
            evidence,
        })?);
        tx.save_job(&job).await.map_err(ApplicationError::storage)?;
        tx.commit().await.map_err(ApplicationError::storage)?;
        Ok(job_view(job, None))
    }
}

pub(super) fn require_current_profile(
    profile: &CandidateProfile,
    revision: u64,
) -> Result<(), ApplicationError> {
    if !profile.consents.processing {
        return Err(ApplicationError::new(
            403,
            "processing_consent_required",
            "Для обработки данных необходимо согласие",
        ));
    }
    if profile.revision != revision {
        return Err(ApplicationError::new(
            409,
            "import_profile_changed",
            "Профиль изменился после загрузки PDF. Загрузите файл заново, чтобы сохранить актуальные данные",
        ));
    }
    Ok(())
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn invalid() -> ApplicationError {
    ApplicationError::new(
        502,
        "resume_extraction_invalid",
        "Не удалось получить достоверные поля из PDF. Повторите загрузку или заполните профиль вручную",
    )
}

/// Whether the quote supports the value: catalog values by their id, label or a common alias as a
/// whole word, numbers only when written out, and text by containment.
fn supported_value(field: &str, value: &str, quote: &str) -> bool {
    let mut aliases: Vec<String> = Vec::new();
    match field {
        "specialization" => {
            let Some(item) = fsp_domain::catalog::catalog()
                .specializations
                .iter()
                .find(|item| item.id.as_str() == value)
            else {
                return false;
            };
            aliases.extend([
                value.replace('_', " "),
                item.label.to_lowercase(),
                item.short.to_lowercase(),
                item.role.to_lowercase(),
            ]);
            aliases.extend(
                match value {
                    "backend" => &["back-end", "бэкенд", "бекенд"][..],
                    "frontend" => &["front-end", "фронтенд"],
                    "fullstack" => &["full-stack", "full stack", "фулстек"],
                    "devops" => &["sre", "девопс"],
                    "onec" => &["1с", "1c"],
                    "qa_automation" | "qa_manual" => &["qa", "тестировщик"],
                    _ => &[],
                }
                .iter()
                .map(|alias| (*alias).to_owned()),
            );
        }
        "selectedGrade" => {
            let Some(grade) = fsp_domain::Grade::ALL
                .into_iter()
                .find(|grade| grade.as_str() == value)
            else {
                return false;
            };
            aliases.extend([value.to_owned(), grade.label().to_lowercase()]);
            aliases.extend(
                match grade {
                    fsp_domain::Grade::Intern => &["стажер", "trainee"][..],
                    fsp_domain::Grade::Middle => &["миддл"],
                    fsp_domain::Grade::Senior => &["синьор", "сениор"],
                    fsp_domain::Grade::Lead => &["тимлид", "team lead", "tech lead", "техлид"],
                    fsp_domain::Grade::Junior => &[],
                }
                .iter()
                .map(|alias| (*alias).to_owned()),
            );
        }
        "stack" => {
            let Some(skill) = fsp_domain::catalog::skill(value) else {
                return false;
            };
            let label = skill.label.to_lowercase();
            aliases.extend([value.replace('_', " "), label.clone()]);
            // «Actix и Axum», «HTML и CSS», «Bash/shell»: any named part counts.
            aliases.extend(
                label
                    .split([',', '/', '(', ')'])
                    .flat_map(|part| part.split(" и "))
                    .map(str::trim)
                    .filter(|part| part.chars().count() >= 2)
                    .map(str::to_owned),
            );
        }
        // Only explicitly stated month totals are imported; never sum overlapping dates.
        "experienceMonths" => {
            return value.parse::<u16>().is_ok_and(|n| n <= 1200)
                && quote
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(2)
                    .any(|words| {
                        words[0] == value
                            && (words[1].starts_with("мес") || words[1].starts_with("month"))
                    });
        }
        // The year someone started in IT, written in the quote.
        "experienceSince" => {
            return value.len() == 4
                && value
                    .parse::<i32>()
                    .is_ok_and(|year| (1950..=current_year()).contains(&year))
                && quote.contains(value);
        }
        _ => return quote.contains(&normalize(value)),
    }
    aliases
        .iter()
        .filter(|alias| !alias.is_empty())
        .any(|alias| {
            quote.match_indices(alias.as_str()).any(|(at, _)| {
                let before = quote[..at].chars().next_back();
                let after = quote[at + alias.len()..].chars().next();
                before.is_none_or(|c| !c.is_alphanumeric())
                    && after.is_none_or(|c| !c.is_alphanumeric())
            })
        })
}

fn validated_preview(
    profile: &CandidateProfile,
    extraction: ResumeExtraction,
    pages: &[PdfPage],
) -> Result<(Value, Vec<ImportEvidence>), ApplicationError> {
    if extraction.fields.is_empty() || extraction.fields.len() > 24 {
        return Err(invalid());
    }
    let mut preview = json!({
        "name": profile.name, "specialization": profile.specialization,
        "selectedGrade": profile.selected_grade, "stack": profile.stack,
        "teamRole": profile.team_role, "experienceMonths": profile.experience_months,
        "experienceSince": profile.experience_since,
        "experience": profile.experience, "softSkills": profile.soft_skills,
    });
    let mut seen = BTreeSet::new();
    let mut evidence = Vec::new();
    for item in extraction.fields {
        let canonical = if matches!(
            item.field.as_str(),
            "specialization" | "selectedGrade" | "stack"
        ) {
            item.value.trim().to_lowercase()
        } else {
            item.value.trim().to_string()
        };
        let value = canonical.as_str();
        let max = match item.field.as_str() {
            "name" | "teamRole" => 120,
            "experience" => 4000,
            "softSkills" => 100,
            "specialization" | "selectedGrade" | "experienceMonths" | "stack" => 40,
            "experienceSince" => 4,
            _ => return Err(invalid()),
        };
        let page = pages
            .iter()
            .find(|page| page.page == item.page)
            .ok_or_else(invalid)?;
        let quote = normalize(&item.quote);
        if value.is_empty()
            || value.chars().count() > max
            || quote.is_empty()
            || item.quote.chars().count() > 4000
            || !normalize(&page.text).contains(&quote)
            || !supported_value(&item.field, value, &quote)
        {
            return Err(invalid());
        }
        let first = seen.insert(item.field.clone());
        if matches!(item.field.as_str(), "stack" | "softSkills") {
            if first {
                preview[&item.field] = json!([]);
            }
            let values = preview[&item.field].as_array_mut().ok_or_else(invalid)?;
            if values.contains(&json!(value))
                || values.len() >= if item.field == "stack" { 5 } else { 12 }
            {
                return Err(invalid());
            }
            values.push(json!(value));
        } else {
            if !first {
                return Err(invalid());
            }
            preview[&item.field] = if item.field == "experienceMonths" {
                json!(value.parse::<u16>().map_err(|_| invalid())?)
            } else if item.field == "experienceSince" {
                // A start year from the résumé replaces stated months.
                preview["experienceMonths"] = Value::Null;
                json!(value.parse::<u16>().map_err(|_| invalid())?)
            } else {
                json!(value)
            };
        }
        evidence.push(ImportEvidence {
            field: item.field,
            page: item.page,
            quote: item.quote,
        });
    }
    // Reuse the authoritative profile bounds and dictionary checks.
    let patch: ProfilePatch = serde_json::from_value(preview.clone()).map_err(|_| invalid())?;
    // An existing incomplete name remains empty until the user fills it in the preview.
    let mut check = patch;
    if check.name.as_deref() == Some("") {
        check.name = None;
    }
    validate_profile_patch(&check).map_err(|_| invalid())?;
    Ok((preview, evidence))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> CandidateProfile {
        CandidateProfile {
            id: "candidate-import".into(),
            alias: "Candidate".into(),
            email: "test@example.com".into(),
            name: "Current name".into(),
            specialization: Specialization::Backend,
            selected_grade: Grade::Middle,
            stack: vec!["postgresql".into()],
            team_role: "Developer".into(),
            experience_months: Some(18),
            experience_since: None,
            experience: "Current experience".into(),
            soft_skills: vec![],
            consents: Consents {
                processing: true,
                publication: false,
            },
            verified_grade: Some(Grade::Junior),
            verified_specialization: Some(Specialization::Backend),
            grade_confirmed_at: None,
            estimate: None,
            auto_fields: Vec::new(),
            revision: 4,
            updated_at: now(),
        }
    }

    #[test]
    fn resume_fields_require_matching_page_quotes_and_preserve_unknown_values() {
        let pages = vec![PdfPage {
            page: 1,
            text: "New name\nRust".into(),
        }];
        let field = fsp_domain::ResumeField {
            field: "name".into(),
            value: "New name".into(),
            page: 1,
            quote: "New name".into(),
        };
        let (preview, evidence) = validated_preview(
            &profile(),
            ResumeExtraction {
                fields: vec![field.clone()],
            },
            &pages,
        )
        .unwrap();
        assert_eq!(preview["name"], "New name");
        assert_eq!(preview["experienceMonths"], 18);
        assert_eq!(preview["stack"], json!(["postgresql"]));
        assert_eq!(evidence[0].page, 1);
        let (capitalized, _) = validated_preview(
            &profile(),
            ResumeExtraction {
                fields: vec![fsp_domain::ResumeField {
                    field: "stack".into(),
                    value: "Rust".into(),
                    page: 1,
                    quote: "Rust".into(),
                }],
            },
            &pages,
        )
        .unwrap();
        assert_eq!(capitalized["stack"], json!(["rust"]));
        for invalid_field in [
            fsp_domain::ResumeField {
                quote: "Invented quote".into(),
                ..field.clone()
            },
            fsp_domain::ResumeField {
                page: 2,
                ..field.clone()
            },
            fsp_domain::ResumeField {
                value: "Invented identity".into(),
                ..field.clone()
            },
            fsp_domain::ResumeField {
                field: "verifiedGrade".into(),
                ..field.clone()
            },
        ] {
            assert!(
                validated_preview(
                    &profile(),
                    ResumeExtraction {
                        fields: vec![invalid_field]
                    },
                    &pages
                )
                .is_err()
            );
        }
        assert!(
            validated_preview(
                &profile(),
                ResumeExtraction {
                    fields: vec![field.clone(), field]
                },
                &pages
            )
            .is_err()
        );
    }

    #[test]
    fn canonical_values_need_actual_support() {
        assert!(supported_value(
            "stack",
            "postgresql",
            "опыт postgresql и rust"
        ));
        assert!(supported_value(
            "specialization",
            "backend",
            "backend-разработчик, middle"
        ));
        assert!(supported_value("stack", "docker", "технология docker."));
        assert!(!supported_value(
            "experienceMonths",
            "60",
            "опыт 160 месяцев"
        ));
        assert!(!supported_value("stack", "rust", "trust me"));
        assert!(!supported_value("selectedGrade", "senior", "разработчик"));
        assert!(!supported_value(
            "experienceMonths",
            "60",
            "5 лет, телефон 6060"
        ));
        assert!(supported_value("experienceMonths", "60", "опыт 60 месяцев"));
        // The whole catalog counts, by id, label or a named part of the label.
        assert!(supported_value(
            "stack",
            "github_actions",
            "ci на github actions"
        ));
        assert!(supported_value("stack", "kafka", "очереди на kafka"));
        assert!(supported_value("stack", "actix", "сервисы на axum"));
        assert!(supported_value(
            "specialization",
            "devops",
            "devops-инженер"
        ));
        assert!(supported_value("selectedGrade", "lead", "тимлид команды"));
        assert!(!supported_value("stack", "go", "google"));
        // A start year must be written in the quote and lie between 1950 and now.
        assert!(supported_value(
            "experienceSince",
            "2016",
            "программирую с 2016 года"
        ));
        assert!(!supported_value(
            "experienceSince",
            "2016",
            "работаю с 2018 года"
        ));
        assert!(!supported_value("experienceSince", "1949", "с 1949 года"));
        assert!(!supported_value("experienceSince", "16", "с 16 лет"));
    }
}
