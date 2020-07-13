CREATE TABLE sessions
(
  key varchar(100) NOT NULL,
  csrf varchar(100) NOT NULL,
  account uuid NOT NULL,
  identity json NOT NULL,
  expiry timestamp,
  invalidated bool NOT NULL DEFAULT FALSE,
  created_at timestamp WITHOUT TIME ZONE DEFAULT (NOW() AT TIME ZONE 'UTC') NOT NULL,
  updated_at timestamp WITHOUT TIME ZONE NULL,
  PRIMARY KEY (key),
  FOREIGN KEY (account) REFERENCES accounts (id) 
);

