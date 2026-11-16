-- HITL 2.0: every task interaction carries an explicit structure. Legacy rows
-- keep working by deriving the kind from the options they already had.
ALTER TABLE task_interactions ADD COLUMN kind TEXT NOT NULL DEFAULT 'TEXT';

UPDATE task_interactions
SET kind = CASE
    WHEN options_json IS NULL OR options_json = '' OR options_json = '[]' THEN 'TEXT'
    WHEN multiple = 1 THEN 'MULTI_CHOICE'
    ELSE 'SINGLE_CHOICE'
END;
