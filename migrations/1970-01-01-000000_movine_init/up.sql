CREATE TABLE movine_migrations (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMP DEFAULT now(),
    updated_at TIMESTAMP DEFAULT now(),
    name TEXT NOT NULL,
    hash TEXT NOT NULL,
    down_sql TEXT
);
