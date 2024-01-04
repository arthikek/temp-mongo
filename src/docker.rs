use std::collections::HashMap;
use bollard::Docker;
use bollard::container::{CreateContainerOptions, Config};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig, PortBinding};
use mongodb::{Client};
use crate::error::TempMongoDockerError;
use futures_util::stream::StreamExt;

pub struct TempMongoDocker {
    docker_client: Docker,
    pub mongo_client: Option<Client>,
}

impl TempMongoDocker {
    pub fn new() -> Result<Self, TempMongoDockerError> {
        let docker_client: Docker;

        #[cfg(windows)]
        {
            // For Windows: Use named pipe defaults
            docker_client = Docker::connect_with_named_pipe_defaults()
                .map_err(|e| TempMongoDockerError::DockerConnectionError(e.to_string()))?;
        }

        #[cfg(unix)]
        {
            // For Unix-based systems: Use Unix socket defaults
            docker_client = Docker::connect_with_unix_defaults()
                .map_err(|e| TempMongoDockerError::DockerConnectionError(e.to_string()))?;
        }


        Ok(TempMongoDocker {
            docker_client,
            mongo_client: None,
        })
    }

    pub async fn create(&mut self) -> Result<(), TempMongoDockerError> {
        let mongo_image = "mongo:latest";

        // Check and pull the image if not present
        let create_image_options = CreateImageOptions {
            from_image: mongo_image,
            ..Default::default()
        };

        let mut create_image_stream = self.docker_client.create_image(Some(create_image_options), None, None);
        while let Some(create_result) = create_image_stream.next().await {
            match create_result {
                Ok(info) => {

                },
                Err(e) => {
                    // Handle the error, return or log it
                    return Err(TempMongoDockerError::DockerConnectionError(e.to_string()));
                }
            }
        }

        // Create container options
        let container_opts = CreateContainerOptions {
            name: "temp_mongo_docker",
            platform: None,
        };
        use std::collections::HashMap;
        use bollard::models::PortBinding;

        let container_config = Config {
            image: Some(mongo_image),
            env: Some(vec!["MONGO_INITDB_ROOT_USERNAME=myuser", "MONGO_INITDB_ROOT_PASSWORD=mypassword"]),
            host_config: Some(HostConfig {
                port_bindings: Some(HashMap::from([
                    ("27017/tcp".to_string(), Some(vec![PortBinding {
                        host_ip: Some("127.0.0.1".to_string()),
                        host_port: Some("27017".to_string()),
                    }])),
                ])),
                ..Default::default()
            }),
            ..Default::default()
        };


        // Creating the container
        match self.docker_client.create_container(Some(container_opts), container_config).await {
            Ok(_) => {
                // Container created successfully, proceed with your logic
            }
            Err(_) => {
                // Error occurred, but we're choosing to ignore it and continue
                // You might want to log this error for debugging purposes
            }
        }

        let uri = "mongodb://127.0.0.1:27017";
// Connecting to MongoDB
        let mongo_client = Client::with_uri_str(uri).await
            .map_err(TempMongoDockerError::MongoConnectionError)?;
        self.mongo_client = Some(mongo_client);

        Ok(())

    }
}
