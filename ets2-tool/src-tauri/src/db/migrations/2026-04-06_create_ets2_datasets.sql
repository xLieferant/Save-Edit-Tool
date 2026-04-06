CREATE TABLE IF NOT EXISTS ets2_countries (
    id TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    game_token TEXT NOT NULL,
    country_code TEXT,
    iso_country_code TEXT,
    country_iso2 TEXT NOT NULL,
    name_en TEXT NOT NULL,
    name_local TEXT NOT NULL,
    aliases_json TEXT NOT NULL DEFAULT '[]',
    coords_json TEXT,
    payment_multiplier REAL NOT NULL DEFAULT 1.0,
    notes_json TEXT NOT NULL DEFAULT '[]',
    source TEXT NOT NULL,
    source_version TEXT NOT NULL DEFAULT 'unknown',
    checksum TEXT NOT NULL,
    warnings_json TEXT NOT NULL DEFAULT '[]',
    dataset_version TEXT NOT NULL,
    imported_at_utc TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ets2_countries_iso2
ON ets2_countries(country_iso2);

CREATE TABLE IF NOT EXISTS ets2_cities (
    id TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    game_token TEXT NOT NULL,
    country_id TEXT NOT NULL,
    country_iso2 TEXT NOT NULL,
    name_en TEXT NOT NULL,
    name_local TEXT NOT NULL,
    aliases_json TEXT NOT NULL DEFAULT '[]',
    population INTEGER,
    coords_json TEXT,
    replaces_city_id TEXT,
    source TEXT NOT NULL,
    source_version TEXT NOT NULL DEFAULT 'unknown',
    checksum TEXT NOT NULL,
    warnings_json TEXT NOT NULL DEFAULT '[]',
    dataset_version TEXT NOT NULL,
    imported_at_utc TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ets2_cities_country
ON ets2_cities(country_iso2, name_en);

CREATE INDEX IF NOT EXISTS idx_ets2_cities_token
ON ets2_cities(game_token);

CREATE TABLE IF NOT EXISTS ets2_companies (
    id TEXT PRIMARY KEY,
    namespace TEXT NOT NULL,
    game_token TEXT NOT NULL,
    name_en TEXT NOT NULL,
    name_local TEXT NOT NULL,
    aliases_json TEXT NOT NULL DEFAULT '[]',
    payment_tier TEXT NOT NULL DEFAULT 'standard',
    payment_multiplier REAL NOT NULL DEFAULT 1.0,
    preferred_cargo_types_json TEXT NOT NULL DEFAULT '[]',
    notes_json TEXT NOT NULL DEFAULT '[]',
    source TEXT NOT NULL,
    source_version TEXT NOT NULL DEFAULT 'unknown',
    checksum TEXT NOT NULL,
    warnings_json TEXT NOT NULL DEFAULT '[]',
    dataset_version TEXT NOT NULL,
    imported_at_utc TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ets2_companies_token
ON ets2_companies(game_token);

CREATE TABLE IF NOT EXISTS ets2_company_offices (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    city_id TEXT,
    city_game_token TEXT NOT NULL,
    prefab_token TEXT,
    source TEXT NOT NULL,
    source_version TEXT NOT NULL DEFAULT 'unknown',
    checksum TEXT NOT NULL,
    warnings_json TEXT NOT NULL DEFAULT '[]',
    dataset_version TEXT NOT NULL,
    imported_at_utc TEXT NOT NULL,
    FOREIGN KEY(company_id) REFERENCES ets2_companies(id)
);

CREATE INDEX IF NOT EXISTS idx_ets2_company_offices_company
ON ets2_company_offices(company_id, city_game_token);

