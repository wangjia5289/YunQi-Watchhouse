ALTER TABLE settings
ADD COLUMN onboarding_completed INTEGER NOT NULL DEFAULT 0
CHECK (onboarding_completed IN (0, 1));
