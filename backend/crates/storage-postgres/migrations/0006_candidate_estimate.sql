-- The automatic profile: the latest estimate from GitHub and FSP evidence and the questionnaire
-- fields it filled. Additive: earlier releases ignore both columns.
ALTER TABLE candidates ADD COLUMN estimate_json TEXT;
ALTER TABLE candidates ADD COLUMN auto_fields_json TEXT NOT NULL DEFAULT '[]'
    CHECK (jsonb_typeof(auto_fields_json::jsonb) = 'array');
