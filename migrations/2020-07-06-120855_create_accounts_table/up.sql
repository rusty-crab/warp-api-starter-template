CREATE TABLE accounts
(
  id uuid NOT NULL,
  email varchar(100) NOT NULL,
  password varchar(150) NOT NULL,
  created_at timestamp WITHOUT TIME ZONE DEFAULT (NOW() AT TIME ZONE 'UTC') NOT NULL,
  updated_at timestamp WITHOUT TIME ZONE NULL,
  PRIMARY KEY (id),
  UNIQUE (email)
);

