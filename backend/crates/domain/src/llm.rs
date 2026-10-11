use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryMatchRequest {
    pub schema_version: &'static str,
    pub repository_reference: String,
    pub repository_facts: String,
    pub competition_reference: String,
    pub competition_facts: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCompetitionMatch {
    pub score: f32,
    pub explanation: String,
    pub evidence_references: Vec<String>,
}
