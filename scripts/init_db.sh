

POSTGRES_PASSWORD="${POSTGRES_PASSWORD:=123}"
POSTGRES_USER="${POSTGRES_USER:=api}"
POSTGRES_DB="${POSTGRES_DB:=judge}"
POSTGRES_PORT="${POSTGRES_PORT:=5432}"

docker rm -f postgres 2>/dev/null || true
docker run --name postgres \
 -e POSTGRES_PASSWORD=${POSTGRES_PASSWORD} \
 -e POSTGRES_DB=${POSTGRES_DB} \
 -e POSTGRES_USER=${POSTGRES_USER} \
 -p ${POSTGRES_PORT}:5432 \
 -d postgres

until pg_isready -d ${POSTGRES_DB} -h localhost -p ${POSTGRES_PORT} -U ${POSTGRES_USER} > /dev/null 2>&1; do
    sleep 1
done

DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}
export DATABASE_URL
sqlx database create
sqlx migrate run