-- The year a candidate started in IT; stated experience counts from it and stays current.
-- Additive: earlier releases ignore the column.
ALTER TABLE candidates ADD COLUMN experience_since INTEGER
    CHECK (experience_since IS NULL OR experience_since BETWEEN 1950 AND 2100);
