-- How careful the agent should be. `standard` keeps the previous behavior,
-- `quick` trims exploration/validation and `careful` keeps full review.
ALTER TABLE tasks ADD COLUMN care_level TEXT NOT NULL DEFAULT 'standard';
