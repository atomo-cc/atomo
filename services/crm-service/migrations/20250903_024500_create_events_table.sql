-- Create Events table for Event Sourcing and Audit Log
-- Generated at: 2025-09-03T02:45:00+00:00

-- Ensure required extension for gen_random_uuid
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Create events table for event sourcing
CREATE TABLE IF NOT EXISTS events (
    event_id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_events_stream_id ON events (stream_id);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events (timestamp);
CREATE INDEX IF NOT EXISTS idx_events_event_type ON events (event_type);
-- Index user_id extracted from JSONB as text; BTREE is appropriate for equality lookups
CREATE INDEX IF NOT EXISTS idx_events_user_id ON events ((metadata->>'user_id'));
CREATE INDEX IF NOT EXISTS idx_events_stream_version ON events (stream_id, version);

-- Unique constraint on stream_id + version for optimistic concurrency
CREATE UNIQUE INDEX IF NOT EXISTS idx_events_stream_version_unique ON events (stream_id, version);

-- Create event_streams table for tracking stream metadata
CREATE TABLE IF NOT EXISTS event_streams (
    stream_id UUID PRIMARY KEY,
    stream_type TEXT NOT NULL,
    current_version BIGINT NOT NULL DEFAULT 0,
    event_count BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_event_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for stream metadata
CREATE INDEX IF NOT EXISTS idx_event_streams_type ON event_streams (stream_type);
CREATE INDEX IF NOT EXISTS idx_event_streams_last_event ON event_streams (last_event_at);

-- Trigger to update stream metadata when events are inserted
CREATE OR REPLACE FUNCTION update_event_stream_metadata()
RETURNS TRIGGER AS $BODY$
BEGIN
    IF EXISTS (SELECT 1 FROM event_streams WHERE stream_id = NEW.stream_id) THEN
        UPDATE event_streams
        SET current_version = GREATEST(current_version, NEW.version),
            event_count = event_streams.event_count + 1,
            last_event_at = NEW.timestamp
        WHERE stream_id = NEW.stream_id;
    ELSE
        INSERT INTO event_streams (stream_id, stream_type, current_version, event_count, last_event_at)
        VALUES (
            NEW.stream_id,
            CASE WHEN position('Created' in NEW.event_type) > 0 THEN substring(NEW.event_type from 1 for position('Created' in NEW.event_type) - 1)
                 WHEN position('Updated' in NEW.event_type) > 0 THEN substring(NEW.event_type from 1 for position('Updated' in NEW.event_type) - 1)
                 WHEN position('Deleted' in NEW.event_type) > 0 THEN substring(NEW.event_type from 1 for position('Deleted' in NEW.event_type) - 1)
                 ELSE 'Unknown'
            END,
            NEW.version,
            1,
            NEW.timestamp
        );
    END IF;

    RETURN NEW;
END;
$BODY$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_event_stream_metadata
    AFTER INSERT ON events
    FOR EACH ROW
    EXECUTE FUNCTION update_event_stream_metadata();

-- Create projection tables for common queries (materialized views)
CREATE TABLE IF NOT EXISTS audit_log_projections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    user_id TEXT,
    user_name TEXT,
    ip_address TEXT,
    user_agent TEXT,
    timestamp TIMESTAMPTZ NOT NULL,
    changes JSONB,
    event_id TEXT NOT NULL REFERENCES events(event_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for audit projections
CREATE INDEX IF NOT EXISTS idx_audit_projections_entity ON audit_log_projections (entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_projections_user ON audit_log_projections (user_id);
CREATE INDEX IF NOT EXISTS idx_audit_projections_timestamp ON audit_log_projections (timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_projections_operation ON audit_log_projections (operation);

-- Function to create audit projections from events
CREATE OR REPLACE FUNCTION create_audit_projection()
RETURNS TRIGGER AS $BODY$
BEGIN
    INSERT INTO audit_log_projections (
        entity_type,
        entity_id,
        operation,
        user_id,
        user_name,
        ip_address,
        user_agent,
        timestamp,
        changes,
        event_id
    )
    VALUES (
        -- Extract entity type from event type
        CASE WHEN position('Created' in NEW.event_type) > 0 
             THEN substring(NEW.event_type from 1 for position('Created' in NEW.event_type) - 1)
             WHEN position('Updated' in NEW.event_type) > 0 
             THEN substring(NEW.event_type from 1 for position('Updated' in NEW.event_type) - 1)
             WHEN position('Deleted' in NEW.event_type) > 0 
             THEN substring(NEW.event_type from 1 for position('Deleted' in NEW.event_type) - 1)
             ELSE 'Unknown'
        END,
        NEW.stream_id::text,
        -- Extract operation from event type
        CASE WHEN NEW.event_type LIKE '%Created' THEN 'Create'
             WHEN NEW.event_type LIKE '%Updated' THEN 'Update'
             WHEN NEW.event_type LIKE '%Deleted' THEN 'Delete'
             ELSE 'Unknown'
        END,
        NEW.metadata->>'user_id',
        -- We'll populate user_name via a separate update
        NULL,
        NEW.metadata->>'ip_address',
        NEW.metadata->>'user_agent',
        NEW.timestamp,
        NEW.payload,
        NEW.event_id
    );
    
    RETURN NEW;
END;
$BODY$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_create_audit_projection
    AFTER INSERT ON events
    FOR EACH ROW
    EXECUTE FUNCTION create_audit_projection();

-- View for easy audit log querying with user names
CREATE VIEW audit_log_view AS
SELECT 
    p.id,
    p.entity_type,
    p.entity_id,
    p.operation,
    p.user_id,
    COALESCE(u.first_name || ' ' || u.last_name, u.email, 'System') as user_name,
    p.ip_address,
    p.user_agent,
    p.timestamp,
    p.changes,
    p.event_id
FROM audit_log_projections p
LEFT JOIN users u ON u.id = p.user_id
ORDER BY p.timestamp DESC;
