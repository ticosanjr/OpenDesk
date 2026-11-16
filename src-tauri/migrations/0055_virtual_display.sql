-- Isolated Xvfb display per computer session.
ALTER TABLE computer_sessions ADD COLUMN display TEXT;
ALTER TABLE computer_sessions ADD COLUMN isolated INTEGER NOT NULL DEFAULT 0;
