CONTAINER_NAME="some-redis"

if [ "$(docker ps -a -q -f name="$CONTAINER_NAME")" ]; then
    docker rm -f $CONTAINER_NAME > /dev/null
    echo "Ending existing image"
fi

echo "Starting docker container" $CONTAINER_NAME
docker run --name some-redis -d -p "6379:6379" redis &> /dev/null
