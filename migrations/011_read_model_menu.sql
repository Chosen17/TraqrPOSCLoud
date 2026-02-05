-- Read model: POS store, menu, categories, items, modifiers, dish yields (all POS local string ids).

-- Store info synced from POS (store_updated).
CREATE TABLE pos_store_sync (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  store_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  local_store_id VARCHAR(255) NOT NULL,
  name VARCHAR(255) NOT NULL,
  timezone VARCHAR(100) NOT NULL DEFAULT 'Europe/London',
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uq_pos_store_sync_device_local (device_id, local_store_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (store_id) REFERENCES stores(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- Menus (from menu_category_created.menu_id).
CREATE TABLE pos_menus (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  local_menu_id VARCHAR(255) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_pos_menus_device_local (device_id, local_menu_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- Menu categories.
CREATE TABLE pos_menu_categories (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  local_menu_id VARCHAR(255) NOT NULL,
  local_category_id VARCHAR(255) NOT NULL,
  name VARCHAR(255) NOT NULL,
  position INT NOT NULL DEFAULT 0,
  image_path VARCHAR(512) NULL,
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uq_pos_menu_categories_device_local (device_id, local_category_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- Menu items.
CREATE TABLE pos_menu_items (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  org_id CHAR(36) NOT NULL,
  device_id CHAR(36) NOT NULL,
  local_item_id VARCHAR(255) NOT NULL,
  local_store_id VARCHAR(255) NULL,
  local_category_id VARCHAR(255) NULL,
  name VARCHAR(255) NOT NULL,
  description TEXT NULL,
  price_pence BIGINT NULL,
  active TINYINT(1) NOT NULL DEFAULT 1,
  image_path VARCHAR(512) NULL,
  customer_editable TINYINT(1) NOT NULL DEFAULT 0,
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uq_pos_menu_items_device_local (device_id, local_item_id),
  FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- Menu item modifiers (menu_item_modifiers_set replaces all for item).
CREATE TABLE pos_menu_item_modifiers (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  device_id CHAR(36) NOT NULL,
  local_menu_item_id VARCHAR(255) NOT NULL,
  name VARCHAR(255) NOT NULL,
  price_delta_pence INT NOT NULL DEFAULT 0,
  position INT NOT NULL DEFAULT 0,
  created_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)),
  UNIQUE KEY uq_pos_modifiers_device_item_position (device_id, local_menu_item_id, position),
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- Dish yields per menu item.
CREATE TABLE pos_dish_yields (
  id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
  device_id CHAR(36) NOT NULL,
  local_menu_item_id VARCHAR(255) NOT NULL,
  estimated_total DECIMAL(18,4) NULL,
  remaining DECIMAL(18,4) NULL,
  warning_threshold DECIMAL(18,4) NULL,
  updated_at DATETIME(3) NOT NULL DEFAULT (CURRENT_TIMESTAMP(3)) ON UPDATE CURRENT_TIMESTAMP(3),
  UNIQUE KEY uq_pos_dish_yields_device_item (device_id, local_menu_item_id),
  FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE INDEX idx_pos_store_sync_org ON pos_store_sync(org_id);
CREATE INDEX idx_pos_menus_device ON pos_menus(device_id);
CREATE INDEX idx_pos_menu_categories_device ON pos_menu_categories(device_id);
CREATE INDEX idx_pos_menu_items_device ON pos_menu_items(device_id);
CREATE INDEX idx_pos_menu_item_modifiers_device ON pos_menu_item_modifiers(device_id);
CREATE INDEX idx_pos_dish_yields_device ON pos_dish_yields(device_id);
