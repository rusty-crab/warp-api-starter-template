# warp-api-starter-template 

This is a starting point for a starter template, this is far from usable from a starter template perspective.
The work is in progress and not recommended for production use at the moment.

## Features

* REST APIs (Warp + http-api-problem + hyper + tokio)
* GraphQL Server with Playground (Juniper)
* Minimal Authentication framework (Argonautica + Biscuit)
* Redis for cache
* PostgreSQL for database and SQLx for query
* systemfd + listenfd to for debug mode auto-reload

Inspired by [Meh's blog post](https://meh.schizofreni.co/2020-04-18/comfy-web-services-in-rust). 

It all started here → [meh/meh.github.io#5](https://github.com/meh/meh.github.io/issues/5#issuecomment-652088596)

## To get started 

Run docker-compose up to get the PostgreSQL, Redis and Adminer running Along with our Web API Service. 
This starts the application in debug mode with auto reload on changes in the source.

```
docker-compose up
```

Migrations are handled using [movine](https://github.com/byronwasti/movine), This is run inside the debug start script.
Adminer instance could be accessed from `http://localhost:8080`. Refer `docker-compose.yml` file for configurations and access credentials.
If you are not using docker-compose to start the application, install movine using `cargo install movine`.

The GraphQL playground could be accessed from `http://localhost:3535/graphql/playground`

To run the application without docker-compose 
```
cp .env.sample .env # make relevant changes to the environment configurations
movine fix # assuming movine is installed, to install movine `cargo install movine`
# run the application in debug mode
RUST_LOG=info cargo run
```

## Release docker build example

```
export DATABASE_URL=postgres://mydb:changeme@192.168.1.4:5432/mydb # this is required for the build to work, this needed for sqlx macros to verify schema
docker build -t warp-api-release:latest -f release.Dockerfile --build-arg DATABASE_URL .
```

### Run the docker example

```
docker run --rm -it -p 3535:3535 --env-file .env -e HOST="0.0.0.0:3535" warp-api-release:latest
```

Refer `.env.sample` file for the env variables required.

## Contributions

With your help we can make this a real good starter template for starting a web service.
Contributions are welcome!

## License

All contributions will be licensed as MIT

