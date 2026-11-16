-- Intuitive recurrence selection, local timezone and notification controls.
ALTER TABLE schedules ADD COLUMN recurrence_json TEXT;
ALTER TABLE schedules ADD COLUMN one_time INTEGER NOT NULL DEFAULT 0;
ALTER TABLE schedules ADD COLUMN notify_signal INTEGER NOT NULL DEFAULT 0;
ALTER TABLE schedules ADD COLUMN notify_signal_recipients TEXT;
ALTER TABLE schedules ADD COLUMN notify_only_on_change INTEGER NOT NULL DEFAULT 0;
ALTER TABLE schedules ADD COLUMN last_output_hash TEXT;
