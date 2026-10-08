-- Additive schema: existing data is never dropped. CamelCase identifiers match
-- the requested database contract and must be quoted in SQL queries.
CREATE TABLE IF NOT EXISTS author (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    birth_year INTEGER,
    death_year INTEGER,
    identity_key TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS publisher (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS license (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS metadata (
    id UUID PRIMARY KEY,
    title TEXT NOT NULL,
    "authorId" UUID REFERENCES author(id),
    "publisherId" UUID REFERENCES publisher(id),
    "LicenseId" UUID REFERENCES license(id),
    date TEXT,
    "numberOfPage" INTEGER CHECK ("numberOfPage" >= 0),
    language TEXT NOT NULL,
    subjects JSONB NOT NULL DEFAULT '[]',
    isbn TEXT
);
-- Upgrade the early bootstrap spelling without changing the relationship/data.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_schema = current_schema() AND table_name = 'metadata' AND column_name = 'licenseId')
       AND NOT EXISTS (SELECT 1 FROM information_schema.columns
                       WHERE table_schema = current_schema() AND table_name = 'metadata' AND column_name = 'LicenseId') THEN
        ALTER TABLE metadata RENAME COLUMN "licenseId" TO "LicenseId";
    END IF;
END $$;
-- Preserve every creator, including order, while authorId identifies the first.
CREATE TABLE IF NOT EXISTS metadata_authors (
    "metadataId" UUID NOT NULL REFERENCES metadata(id) ON DELETE CASCADE,
    "authorId" UUID NOT NULL REFERENCES author(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY ("metadataId", position)
);
CREATE TABLE IF NOT EXISTS schemas (
    id UUID PRIMARY KEY,
    type TEXT NOT NULL,
    scope TEXT NOT NULL,
    tokenizer TEXT,
    scan JSONB NOT NULL,
    result JSONB NOT NULL,
    "updatedAt" TIMESTAMPTZ NOT NULL
);
CREATE TABLE IF NOT EXISTS books (
    id UUID PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('pending', 'scanned')),
    "userId" TEXT,
    "metadataId" UUID NOT NULL UNIQUE REFERENCES metadata(id),
    "schemaId" UUID UNIQUE REFERENCES schemas(id),
    token BIGINT CHECK (token >= 0),
    score DOUBLE PRECISION CHECK (score BETWEEN 0 AND 1),
    source TEXT NOT NULL CHECK (source IN ('upload', 'gutenberg')),
    gutenberg_id BIGINT,
    format TEXT NOT NULL DEFAULT 'epub',
    blob JSONB,
    "createdAt" TIMESTAMPTZ NOT NULL,
    "updatedAt" TIMESTAMPTZ NOT NULL,
    CHECK ((source = 'gutenberg') = (gutenberg_id IS NOT NULL)),
    CHECK ((status = 'scanned') = ("schemaId" IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS books_gutenberg_unique
    ON books (gutenberg_id) WHERE source = 'gutenberg';
CREATE INDEX IF NOT EXISTS books_created_desc ON books ("createdAt" DESC);
