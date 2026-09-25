-- Approved packs not pulled into folderskin-community yet, which GET /v1/exports/pending counts
-- for anyone who asks (src/publish.ts): from this index alone, rather than by reading every pack
-- ever approved, since an approved pack stays approved once it has been pulled.
CREATE INDEX exports_pending ON submissions (status, exported_at);
