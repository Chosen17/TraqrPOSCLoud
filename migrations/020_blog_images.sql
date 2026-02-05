-- Optional featured image for blog posts; uploads stored under UPLOAD_DIR/blogs/

ALTER TABLE blogs ADD COLUMN featured_image_path VARCHAR(500) NULL AFTER body;
