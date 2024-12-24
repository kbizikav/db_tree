# Setup

```
cp .env.example .env
docker run --name postgres-db -e POSTGRES_PASSWORD=password -e POSTGRES_DB=db -p 5432:5432 -d postgres
```

## Migration

```bash
sqlx database reset -y && sqlx database setup
```