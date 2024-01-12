use assert2::{assert, let_assert};
use mongodb::bson::{doc, Document};
use temp_mongo::TempMongo;
use temp_mongo::TempMongoDocker;

#[cfg_attr(feature = "tokio-runtime", tokio::test)]
#[cfg_attr(feature = "async-std-runtime", async_std::test)]
async fn insert_and_find() {
    let_assert!(Ok(mongo) = TempMongo::new().await);
    let database = mongo.client().database("test");
    let collection = database.collection::<Document>("foo");

    let_assert!(Ok(id) = collection.insert_one(doc! { "hello": "world" }, None).await);
    let_assert!(Some(id) = id.inserted_id.as_object_id());
    let_assert!(Ok(Some(document)) = collection.find_one(doc! { "_id": id }, None).await);
    assert!(document == doc! { "_id": id, "hello": "world" });

    // Not needed, but shows better errors.
    assert!(let Ok(()) = mongo.kill_and_clean().await);
}


#[cfg_attr(feature = "tokio-runtime", tokio::test)]
#[cfg_attr(feature = "async-std-runtime", async_std::test)]
async fn test_temp_mongo_docker() {
    // Initialize TempMongoDocker
    let mut temp_mongo_docker = TempMongoDocker::new().expect("Failed to create TempMongoDocker");

    // Create a MongoDB container
    let_assert!(Ok(_container) = temp_mongo_docker.create().await);

    // Assuming `create` also initializes `mongo_client`
    let mongo_client = temp_mongo_docker
        .mongo_client
        .as_ref()
        .expect("MongoDB client not initialized");
    let database = mongo_client.database("test");
    let collection = database.collection::<Document>("foo");

    // Insert a document
    let_assert!(
        Ok(id) = collection
            .insert_one(doc! { "hello": "docker world" }, None)
            .await
    );
    let_assert!(Some(id) = id.inserted_id.as_object_id());

    // Find the inserted document
    let_assert!(Ok(Some(document)) = collection.find_one(doc! { "_id": id }, None).await);
    assert!(document == doc! { "_id": id, "hello": "docker world" });
}


#[cfg_attr(feature = "tokio-runtime", tokio::test)]
#[cfg_attr(feature = "async-std-runtime", async_std::test)]
async fn test_container_status() {
    // Create a TempMongoDocker instance
    let mut temp_mongo_docker = TempMongoDocker::new().expect("Failed to create TempMongoDocker");

    // Set up the environment (assuming this creates a container named "temp_mongo_docker")
    temp_mongo_docker
        .create()
        .await
        .expect("Failed to create environment");

    // Test the container_status function
    let status = temp_mongo_docker.container_status().await;
    match status {
        Ok(status) => assert!(status,"The container is found!"),
        Err(error) => panic!("Error while checking container status: {:?}", error),
    }
}
