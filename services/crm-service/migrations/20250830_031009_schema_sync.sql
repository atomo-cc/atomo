-- Auto-generated migration
-- Generated at: 2025-08-30T03:10:09.915536700+00:00

-- Create table for Contact
CREATE TABLE contact (
    notes JSONB NOT NULL,
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL,
    createdAt TIMESTAMPTZ NOT NULL,
    tags JSONB NOT NULL,
    firstName TEXT NOT NULL,
    phone TEXT,
    updatedAt TIMESTAMPTZ NOT NULL,
    companyId TEXT,
    lastName TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_contact_created_at ON contact (created_at);
CREATE INDEX idx_contact_updated_at ON contact (updated_at);

-- Create table for Company
CREATE TABLE company (
    address TEXT,
    industry TEXT,
    name TEXT NOT NULL,
    size TEXT,
    updatedAt TIMESTAMPTZ NOT NULL,
    website TEXT,
    notes JSONB NOT NULL,
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    createdAt TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_company_created_at ON company (created_at);
CREATE INDEX idx_company_updated_at ON company (updated_at);

-- Create table for Deal
CREATE TABLE deal (
    companyId TEXT,
    title TEXT NOT NULL,
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    actualCloseDate TIMESTAMPTZ,
    value NUMERIC NOT NULL,
    stage TEXT NOT NULL,
    contactId TEXT NOT NULL,
    description JSONB NOT NULL,
    updatedAt TIMESTAMPTZ NOT NULL,
    createdAt TIMESTAMPTZ NOT NULL,
    expectedCloseDate TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_deal_created_at ON deal (created_at);
CREATE INDEX idx_deal_updated_at ON deal (updated_at);

-- Create table for ParagraphBlock
CREATE TABLE paragraphblock (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_paragraphblock_created_at ON paragraphblock (created_at);
CREATE INDEX idx_paragraphblock_updated_at ON paragraphblock (updated_at);

-- Create table for CallLogBlock
CREATE TABLE calllogblock (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type TEXT NOT NULL,
    outcome TEXT NOT NULL,
    notes TEXT NOT NULL,
    recordedAt TIMESTAMPTZ NOT NULL,
    duration NUMERIC NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_calllogblock_created_at ON calllogblock (created_at);
CREATE INDEX idx_calllogblock_updated_at ON calllogblock (updated_at);

-- Create table for MeetingNoteBlock
CREATE TABLE meetingnoteblock (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type TEXT NOT NULL,
    actionItems JSONB NOT NULL,
    attendees JSONB NOT NULL,
    meetingDate TIMESTAMPTZ NOT NULL,
    notes TEXT NOT NULL,
    agenda TEXT NOT NULL,
    title TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_meetingnoteblock_created_at ON meetingnoteblock (created_at);
CREATE INDEX idx_meetingnoteblock_updated_at ON meetingnoteblock (updated_at);

-- Create table for TaskBlock
CREATE TABLE taskblock (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    assignedTo TEXT,
    type TEXT NOT NULL,
    completed BOOLEAN NOT NULL,
    description TEXT,
    dueDate TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_taskblock_created_at ON taskblock (created_at);
CREATE INDEX idx_taskblock_updated_at ON taskblock (updated_at);

-- DROP TABLE IF EXISTS _atomo_migrations; -- Uncomment to drop unused table
