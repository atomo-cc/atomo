-- Migration: add Deal.position and create Activity table

-- Add position column for deal Kanban ordering
ALTER TABLE deal ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

-- Initialize position per stage using row_number over partition
WITH ranked AS (
  SELECT id, stage, ROW_NUMBER() OVER (PARTITION BY stage ORDER BY updated_at DESC, created_at DESC) - 1 AS rn
  FROM deal
)
UPDATE deal d
SET position = r.rn
FROM ranked r
WHERE d.id = r.id;

-- Index to support stage-ordered queries
CREATE INDEX IF NOT EXISTS idx_deal_stage_position ON deal (stage, position);

-- Create activity table
CREATE TABLE activity (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    contact_id TEXT NOT NULL,
    activity_type TEXT NOT NULL,
    title TEXT,
    content TEXT,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_activity_contact_id ON activity (contact_id);
CREATE INDEX IF NOT EXISTS idx_activity_created_at ON activity (created_at DESC);
