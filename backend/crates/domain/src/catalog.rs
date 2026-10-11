//! The platform catalog: grades, specializations and skills (`data/catalog.json`).
//!
//! One data file serves the Rust API, the contract generator and the development mock, so the
//! vocabulary cannot drift between them. Specializations name the two topics their knowledge test
//! covers and the skills that indicate them in a candidate's evidence.

use std::sync::LazyLock;

use serde::Deserialize;

use crate::{Grade, Specialization};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeInfo {
    pub id: Grade,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecializationInfo {
    pub id: Specialization,
    pub label: String,
    pub short: String,
    /// The person's title, e.g. «Разработчик 1С».
    pub role: String,
    pub group: String,
    /// The two skills a knowledge test for this specialization covers.
    pub test_topics: [String; 2],
    /// Skills whose evidence points to this specialization.
    pub indicators: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub id: String,
    pub label: String,
    pub category: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub version: String,
    pub industries: Vec<CatalogEntry>,
    pub grades: Vec<GradeInfo>,
    pub specialization_groups: Vec<CatalogEntry>,
    pub skill_categories: Vec<CatalogEntry>,
    pub specializations: Vec<SpecializationInfo>,
    pub skills: Vec<SkillInfo>,
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let catalog: Catalog = serde_json::from_str(include_str!("../data/catalog.json"))
        .expect("the catalog must be valid");
    validate(&catalog).expect("the catalog must be consistent");
    catalog
});

/// Every grade and specialization once and in enum order, and every skill reference known.
fn validate(catalog: &Catalog) -> Result<(), String> {
    let grades = catalog
        .grades
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    if grades != Grade::ALL {
        return Err("grades must list every grade in order".into());
    }
    let specializations = catalog
        .specializations
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    if specializations.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("specializations must follow the enum order once each".into());
    }
    for specialization in &catalog.specializations {
        if !catalog
            .specialization_groups
            .iter()
            .any(|group| group.id == specialization.group)
        {
            return Err(format!("unknown group of {}", specialization.id.as_str()));
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    for skill in &catalog.skills {
        if !ids.insert(skill.id.as_str()) {
            return Err(format!("duplicate skill {}", skill.id));
        }
        if !catalog
            .skill_categories
            .iter()
            .any(|category| category.id == skill.category)
        {
            return Err(format!("unknown category of {}", skill.id));
        }
    }
    for specialization in &catalog.specializations {
        for skill in specialization
            .test_topics
            .iter()
            .chain(&specialization.indicators)
        {
            if !ids.contains(skill.as_str()) {
                return Err(format!(
                    "{} names unknown skill {skill}",
                    specialization.id.as_str()
                ));
            }
        }
    }
    Ok(())
}

#[must_use]
pub fn catalog() -> &'static Catalog {
    &CATALOG
}

#[must_use]
pub fn is_skill(id: &str) -> bool {
    skill(id).is_some()
}

#[must_use]
pub fn skill(id: &str) -> Option<&'static SkillInfo> {
    CATALOG.skills.iter().find(|item| item.id == id)
}

/// The skill's label, or the id itself for an unknown skill.
#[must_use]
pub fn skill_label(id: &str) -> &str {
    skill(id).map_or(id, |item| item.label.as_str())
}

#[must_use]
pub fn specialization(id: Specialization) -> &'static SpecializationInfo {
    CATALOG
        .specializations
        .iter()
        .find(|item| item.id == id)
        .expect("validated: every specialization is in the catalog")
}

#[must_use]
pub fn grade_label(grade: Grade) -> &'static str {
    CATALOG
        .grades
        .iter()
        .find(|item| item.id == grade)
        .map(|item| item.label.as_str())
        .expect("validated: every grade is in the catalog")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_is_consistent_and_covers_every_enum_value() {
        let catalog = catalog();
        assert_eq!(catalog.version, "catalog-v2");
        assert_eq!(catalog.grades.len(), Grade::ALL.len());
        assert_eq!(catalog.specializations.len(), 25);
        assert!(catalog.skills.len() >= 250);
        assert_eq!(Grade::Lead.label(), "Лид");
        assert_eq!(Specialization::Onec.label(), "Разработка на 1С");
        assert_eq!(
            specialization(Specialization::Onec).test_topics,
            ["onec_language".to_owned(), "onec_queries".to_owned()]
        );
        assert!(is_skill("onec_language") && is_skill("abap") && is_skill("rust"));
        assert_eq!(skill_label("onec_skd"), "СКД (Система компоновки данных)");
    }

    #[test]
    fn every_specialization_round_trips_through_its_id() {
        for item in &catalog().specializations {
            let parsed: Specialization =
                serde_json::from_value(serde_json::json!(item.id.as_str())).unwrap();
            assert_eq!(parsed, item.id);
        }
    }
}
