# warp-api-starter-template 

Inspired by [Meh's blog post](https://meh.schizofreni.co/2020-04-18/comfy-web-services-in-rust). 

It all started here → [meh/meh.github.io#5](https://github.com/meh/meh.github.io/issues/5#issuecomment-652088596)

# To get started
Run docker-compose up to get the PostgreSQL, Redis and Adminer running. 
```
docker-compose up
```
Adminer could be accessed from `http://localhost:8080`. Refer `docker-compose.yml` file for configurations and access credentials.
Create the schema using the `migrations/v1__schema.sql`. Plan is to completely dockerize and automate this step using [movine](https://github.com/byronwasti/movine).
Since we will require the PostgreSQL up and running with the schema intact in order to build the source as SQLx run the SQL validations with a running database.
```
cp .env.sample .env
RUST_LOG=info cargo run
```
The GraphQL playground could be accessed from `http://localhost:3535/graphql/playground`

