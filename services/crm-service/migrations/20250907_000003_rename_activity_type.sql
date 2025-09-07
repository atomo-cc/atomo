-- Rename column 'type' to 'activity_type' if needed
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name='activity' AND column_name='type'
    ) THEN
        ALTER TABLE activity RENAME COLUMN type TO activity_type;
    END IF;
END$$;

