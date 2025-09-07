-- Align deal.position type with generated NUMERIC mapping
ALTER TABLE deal 
    ALTER COLUMN position TYPE NUMERIC USING position::numeric;

-- Optional: add check to enforce non-negative positions
ALTER TABLE deal 
    ADD CONSTRAINT deal_position_nonnegative CHECK (position >= 0);

