-- Blogs: owner and manager can create. SEO-friendly slug.

CREATE TABLE IF NOT EXISTS blogs (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  title VARCHAR(500) NOT NULL,
  slug VARCHAR(500) NOT NULL,
  excerpt TEXT NULL,
  body LONGTEXT NOT NULL,
  author_id CHAR(36) NOT NULL,
  published_at DATETIME(3) NULL COMMENT 'NULL = draft',
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uq_blogs_slug (slug),
  FOREIGN KEY (author_id) REFERENCES cloud_users(id) ON DELETE CASCADE
);

CREATE INDEX idx_blogs_published ON blogs(published_at);
CREATE INDEX idx_blogs_author ON blogs(author_id);
