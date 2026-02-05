-- Simple session for portal: login sets cookie, API can resolve current user.

CREATE TABLE IF NOT EXISTS cloud_sessions (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  user_id CHAR(36) NOT NULL,
  token VARCHAR(64) NOT NULL,
  expires_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_cloud_sessions_token (token),
  FOREIGN KEY (user_id) REFERENCES cloud_users(id) ON DELETE CASCADE
);

CREATE INDEX idx_cloud_sessions_token ON cloud_sessions(token);
CREATE INDEX idx_cloud_sessions_expires ON cloud_sessions(expires_at);
