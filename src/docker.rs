use crate::error::TempMongoDockerError;
use crate::util::PortGenerator;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig, PortBinding};
use bollard::Docker;
use futures_util::stream::StreamExt;
use mongodb::Client;
use std::collections::HashMap;
use std::string::String;
use tokio::time::sleep;

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
    name_container: Option<String>,
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
            name_container: None,
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
            name: "",
            platform: Some("linux/amd64"),
        };

        let port = PortGenerator::new().generate().selected_port().unwrap();
        let container_config = Config {
            image: Some("mongo:latest"),

            cmd: Some(vec!["mongod", "--noauth"]),
            host_config: Some(HostConfig {
                port_bindings: Some(HashMap::from([(
                    "27017/tcp".to_string(),
                    Some(vec![PortBinding {
                        host_ip: Some("127.0.0.1".to_string()),
                        host_port: Some(port.to_string()),
                    }]),
                )])),
                ..Default::default()
            }),
            ..Default::default()
        };

        self.start_container(container_opts, container_config)
            .await
            .unwrap();

        let uri = format!(
            "mongodb://127.0.0.1:{}/messenger?directConnection=true",
            port
        );
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

    /// create a new Docker container using the provided options and
    /// configuration parameters.
    async fn start_container(
        &mut self,
        container_opts: CreateContainerOptions<&str>,
        container_config: Config<&str>,
    ) -> Result<(), TempMongoDockerError> {
        let create_response = self
            .docker_client
            .create_container(Some(container_opts), container_config)
            .await
            .map_err(TempMongoDockerError::BollardConnectionError)?;

        // Container ID from the create response
        self.name_container = Some(create_response.id);

        let container_name = match self.name_container {
            Some(ref name) => name,
            None => return Err(TempMongoDockerError::ContainerNameNotSet),
        };

        // Now start the container using the container ID
        self.docker_client
            .start_container(container_name, None::<StartContainerOptions<String>>)
            .await
            .map_err(TempMongoDockerError::BollardConnectionError)?;

        sleep(std::time::Duration::from_millis(50)).await;
        Ok(())
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

    /// Stops the MongoDB container and removes it.
    pub async fn kill_and_clean(&mut self) -> Result<(), TempMongoDockerError> {
        let container_name = match self.name_container {
            Some(ref name) => name,
            None => return Err(TempMongoDockerError::ContainerNameNotSet),
        };

        self.docker_client
            .stop_container(container_name, None)
            .await
            .map_err(TempMongoDockerError::BollardConnectionError)?;

        // Now remove the container
        self.docker_client
            .remove_container(container_name, None)
            .await
            .map_err(TempMongoDockerError::BollardConnectionError)?;

        Ok(())
    }
    ///Stops the docker container however retains the information in the docker container.
    pub async fn kill_not_clean(&mut self) -> Result<(), TempMongoDockerError> {
        let container_name = match self.name_container {
            Some(ref name) => name,
            None => return Err(TempMongoDockerError::ContainerNameNotSet),
        };

        self.docker_client
            .stop_container(container_name, None)
            .await
            .map_err(TempMongoDockerError::BollardConnectionError)?;

        Ok(())
    }
}
