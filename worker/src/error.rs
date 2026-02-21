use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecError {
    #[error("Datbase error :{0}")]
    DatabaseError(
        #[from]
        #[source]
        sqlx::Error,
    ),

    #[error("Docker error :{0}")]
    DockerError(
        #[from]
        #[source]
        bollard::errors::Error,
    ),

    #[error("RabbitMQ error :{0}")]
    QueueError(
        #[from]
        #[source]
        lapin::Error,
    ),

    #[error("Error parsing rabbitmq message")]
    ParseError,
    #[error("")]
    PoolError(#[from] deadpool::managed::PoolError<bollard::errors::Error>),
}
