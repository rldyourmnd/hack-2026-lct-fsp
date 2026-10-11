CREATE TABLE IF NOT EXISTS graph_interviews (
    id text PRIMARY KEY,
    candidate_id text NOT NULL,
    status text NOT NULL CHECK (status IN ('active', 'completed')),
    revision bigint NOT NULL CHECK (revision >= 0),
    state jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS graph_interviews_candidate_created_idx
    ON graph_interviews (candidate_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS graph_interviews_one_active_candidate_idx
    ON graph_interviews (candidate_id) WHERE status = 'active';
