-- Docs: guides and setup/usage (Stripe-style sidebar + breadcrumbs).
-- section = sidebar group (e.g. "Getting started"); sort_order for ordering within section.

CREATE TABLE IF NOT EXISTS docs (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  title VARCHAR(500) NOT NULL,
  slug VARCHAR(500) NOT NULL,
  body LONGTEXT NOT NULL,
  section VARCHAR(200) NOT NULL DEFAULT 'General',
  sort_order INT NOT NULL DEFAULT 0,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uq_docs_slug (slug)
);

CREATE INDEX idx_docs_section ON docs(section);
CREATE INDEX idx_docs_sort ON docs(section, sort_order);
