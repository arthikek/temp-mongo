use crate::error::TempMongoDockerError;
use bollard::container::{Config, CreateContainerOptions, ListContainersOptions};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig, PortBinding};
use bollard::Docker;
use futures_util::stream::StreamExt;
use mongodb::Client;
use std::collections::HashMap;
use std::time::Duration;

/// A utility for creating and managing a temporary MongoDB instance within a Docker container.
///
/// This struct provides functionalities to create a Docker container running MongoDB,
/// connect to it, and check its status, making it suitable for use in testing environments
/// where a MongoDB instance is required.
pub struct TempMongoDocker {
    /// Client to interact with the Docker daemon.
    docker_client: Docker,
    /// MongoDB client for performing operations against the MongoDB instance within the container.
    pub mongo_client: Option<Client>,
}

impl TempMongoDocker {
    /// Constructs a new `TempMongoDocker`.
    ///
    /// Establishes a connection with the Docker daemon using platform-specific methods.
    /// Returns an instance of `TempMongoDocker` or an error if the connection fails.
    pub fn new() -> Result<Self, TempMongoDockerError> {
        let docker_client: Docker;

        #[cfg(windows)]
        {
            docker_client = Docker::connect_with_named_pipe_defaults()
                .map_err(|e| TempMongoDockerError::BollardConnectionError(e))?;
        }
        #[cfg(unix)]
        {
            docker_client = Docker::connect_with_socket_defaults()
                .map_err(|e| TempMongoDockerError::BollardConnectionError(e))?;
        }

        Ok(TempMongoDocker {
            docker_client,
            mongo_client: None,
        })
    }

    /// Creates a MongoDB container with predefined configuration.
    ///
    /// If the MongoDB container does not already exist, this function will create and start one.
    /// It then establishes a connection to the MongoDB instance running in the container.
    /// Returns `Ok` if successful, or an error in case of any failures.
    pub async fn create(&mut self) -> Result<(), TempMongoDockerError> {
        self.setup_image().await?;

        let container_opts = CreateContainerOptions {
            name: "temp_mongo_docker",
            platform: None,
        };

        let container_config = Config {
            image: Some("mongo:latest"),
            env: Some(vec![
                "MONGO_INITDB_ROOT_USERNAME=myuser",
                "MONGO_INITDB_ROOT_PASSWORD=mypassword",
            ]),
            host_config: Some(HostConfig {
                port_bindings: Some(HashMap::from([(
                    "27017/tcp".to_string(),
                    Some(vec![PortBinding {
                        host_ip: Some("127.0.0.1".to_string()),
                        host_port: Some("27017".to_string()),
                    }]),
                )])),
                ..Default::default()
            }),
            ..Default::default()
        };
        //Check this error handeling
        self.check_and_create_container(container_opts, container_config)
            .await
            .unwrap();

        let uri = "mongodb://127.0.0.1:27017/messenger?directConnection=true";
        self.mongo_client = Some(
            Client::with_uri_str(uri)
                .await
                .map_err(TempMongoDockerError::MongoConnectionError)?,
        );

        Ok(())
    }

    /// Checks the status of the Docker container and creates it if it does not exist.
    /// This function ensures that only one instance of the specified container is running.
    /// In case of a naming conflict (HTTP 409 error), it waits briefly and continues,
    /// assuming that another process is already creating the container.
    /// Returns an error if the container creation fails for reasons other than a naming conflict.
    async fn check_and_create_container(
        &mut self,
        container_opts: CreateContainerOptions<&str>,
        container_config: Config<&str>,
    ) -> Result<(), TempMongoDockerError> {
        let retry_wait_duration = Duration::from_millis(50);

        if !self.container_status().await? {
            match self
                .create_container(container_opts, container_config)
                .await
            {
                Some(Ok(_)) => Ok(()),
                Some(Err(TempMongoDockerError::BollardConnectionError(
                    bollard::errors::Error::DockerResponseServerError { status_code, .. },
                ))) if status_code == 409 => {
                    tokio::time::sleep(retry_wait_duration).await;
                    Ok(())
                }
                Some(Err(e)) => Err(e),
                None => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    /// Checks if the MongoDB container is currently running.
    ///
    /// Returns `Ok(true)` if the container is running, `Ok(false)` if not, and an error
    /// if there is a problem checking the container status.
    pub async fn container_status(&self) -> Result<bool, TempMongoDockerError> {
        let options = ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        };

        let container_list_future = self.docker_client.list_containers(Some(options));
        match container_list_future.await {
            Ok(containers) => Ok(containers.iter().any(|container| {
                container
                    .names
                    .iter()
                    .flatten()
                    .any(|name| name.trim_start_matches('/') == "temp_mongo_docker")
            })),
            Err(error) => Err(TempMongoDockerError::DockerConnectionError(
                error.to_string(),
            )),
        }
    }

    /// create a new Docker container using the provided options and
    /// configuration parameters.
    async fn create_container(
        &mut self,
        container_opts: CreateContainerOptions<&str>,
        container_config: Config<&str>,
    ) -> Option<Result<(), TempMongoDockerError>> {
        match self
            .docker_client
            .create_container(Some(container_opts), container_config)
            .await
        {
            Ok(response) => {
                println!("Container created with id {}", response.id);
                Some(Ok(()))
            }
            Err(error) => Some(Err(TempMongoDockerError::BollardConnectionError(error))),
        }
    }

    /// Pulls the latest MongoDB image from the Docker registry.
    /// This function checks for the latest MongoDB image and pulls it if not present.
    /// Returns `Ok` with the image name if successful, or an error otherwise.
    async fn setup_image(&mut self) -> Result<&'static str, TempMongoDockerError> {
        let mongo_image = "mongo:latest";

        let create_image_options = CreateImageOptions {
            from_image: mongo_image,
            ..Default::default()
        };

        let mut create_image_stream =
            self.docker_client
                .create_image(Some(create_image_options), None, None);
        while let Some(create_result) = create_image_stream.next().await {
            match create_result {
                Ok(_) => {}
                Err(e) => return Err(TempMongoDockerError::DockerConnectionError(e.to_string())),
            }
        }

        Ok(mongo_image)
    }
}
