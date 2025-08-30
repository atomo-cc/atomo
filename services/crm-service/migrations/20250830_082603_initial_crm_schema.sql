-- Auto-generated migration
-- Generated at: 2025-08-30T08:26:03.771525700+00:00

-- Create table for Contact
CREATE TABLE contact (
    phone TEXT,
    first_name TEXT NOT NULL,
    company_id TEXT,
    last_name TEXT NOT NULL,
    notes JSONB NOT NULL,
    tags JSONB NOT NULL,
    email TEXT NOT NULL,
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_contact_created_at ON contact (created_at);
CREATE INDEX idx_contact_updated_at ON contact (updated_at);

-- Create table for Company
CREATE TABLE company (
    notes JSONB NOT NULL,
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    website TEXT,
    size TEXT,
    address TEXT,
    industry TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_company_created_at ON company (created_at);
CREATE INDEX idx_company_updated_at ON company (updated_at);

-- Create table for Deal
CREATE TABLE deal (
    description JSONB NOT NULL,
    actual_close_date TIMESTAMPTZ,
    value NUMERIC NOT NULL,
    company_id TEXT,
    stage TEXT NOT NULL,
    title TEXT NOT NULL,
    expected_close_date TIMESTAMPTZ,
    contact_id TEXT NOT NULL,
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
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
    duration NUMERIC NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    type TEXT NOT NULL,
    notes TEXT NOT NULL,
    outcome TEXT NOT NULL,
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
    title TEXT NOT NULL,
    action_items JSONB NOT NULL,
    notes TEXT NOT NULL,
    meeting_date TIMESTAMPTZ NOT NULL,
    attendees JSONB NOT NULL,
    agenda TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_meetingnoteblock_created_at ON meetingnoteblock (created_at);
CREATE INDEX idx_meetingnoteblock_updated_at ON meetingnoteblock (updated_at);

-- Create table for TaskBlock
CREATE TABLE taskblock (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    description TEXT,
    type TEXT NOT NULL,
    assigned_to TEXT,
    due_date TIMESTAMPTZ,
    title TEXT NOT NULL,
    completed BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_taskblock_created_at ON taskblock (created_at);
CREATE INDEX idx_taskblock_updated_at ON taskblock (updated_at);

-- Create table for CompanySize
CREATE TABLE companysize (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    _enum_type TEXT NOT NULL,
    _enum_value_1 TEXT NOT NULL,
    _enum_value_3 TEXT NOT NULL,
    _enum_value_4 TEXT NOT NULL,
    _enum_value_2 TEXT NOT NULL,
    _enum_value_0 TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_companysize_created_at ON companysize (created_at);
CREATE INDEX idx_companysize_updated_at ON companysize (updated_at);

-- Create table for DealStage
CREATE TABLE dealstage (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    _enum_value_1 TEXT NOT NULL,
    _enum_value_5 TEXT NOT NULL,
    _enum_value_0 TEXT NOT NULL,
    _enum_value_3 TEXT NOT NULL,
    _enum_type TEXT NOT NULL,
    _enum_value_4 TEXT NOT NULL,
    _enum_value_2 TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_dealstage_created_at ON dealstage (created_at);
CREATE INDEX idx_dealstage_updated_at ON dealstage (updated_at);

