CREATE TABLE accounts
(
  id uuid NOT NULL,
  email varchar(100) NOT NULL,
  password varchar(150) NOT NULL,
  created_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at timestamp NULL,
  PRIMARY KEY (id)
);

CREATE TABLE sessions
(
  key varchar(100) NOT NULL,
  csrf varchar(100) NOT NULL,
  account uuid NOT NULL,
  identity json NOT NULL,
  expiry timestamp,
  invalidated bool NOT NULL DEFAULT 0,
  created_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at timestamp NULL,
  PRIMARY KEY (id)
);
