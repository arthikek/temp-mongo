use assert2::{assert, let_assert};
use futures_util::stream::TryStreamExt;
use mongodb::bson::{doc, Document};

use temp_mongo::TempMongo;
use temp_mongo::TempMongoDocker;

//Testing if we can upload a normal document and retrieve it from the temporary database
//In addition to this we are also testing if the database is truly erased from the system by making use of kill_and_clean
#[cfg_attr(feature = "tokio-runtime", tokio::test)]
#[cfg_attr(feature = "async-std-runtime", async_std::test)]
async fn insert_and_find() -> mongodb::error::Result<()> {
    let_assert!(Ok(mongo) = TempMongo::new().await);

    let database = mongo.client().database("test");
    let collection = database.collection::<Document>("foo");

    let_assert!(Ok(id) = collection.insert_one(doc! { "hello": "world" }, None).await);
    let_assert!(Some(id) = id.inserted_id.as_object_id());
    let_assert!(Ok(Some(document)) = collection.find_one(doc! { "_id": id }, None).await);
    assert_eq!(document, doc! { "_id": id, "hello": "world" });

    // Clean up the temporary MongoDB instance
    assert!(let Ok(()) = mongo.kill_and_clean().await);

    // Connect to the MongoDB instance with a new client
    let_assert!(Ok(mongo_2) = TempMongo::new().await);
    let new_client = mongo_2.client();
    // Check if the 'test' database still exists
    let db_names = new_client.list_database_names(None, None).await?;
    if db_names.contains(&"test".to_string()) {
        panic!("Database 'test' should be deleted, but it still exists");
    }
    assert!(let Ok(()) = mongo_2.kill_and_clean().await);
    Ok(())
}

//Testing spawning asynchronous threads
#[cfg_attr(feature = "tokio-runtime", tokio::test)]
#[cfg_attr(feature = "async-std-runtime", async_std::test)]
async fn insert_and_find_multiple_instances() {
    let instance_count = 5;

    let handles = (0..instance_count)
        .map(|_| {
            tokio::spawn(async move {
                let_assert!(Ok(mongo) = TempMongo::new().await);
                let database = mongo.client().database("test_1");
                let collection = database.collection::<Document>("foo");

                let_assert!(Ok(id) = collection.insert_one(doc! { "hello": "world" }, None).await);
                let_assert!(Some(id) = id.inserted_id.as_object_id());
                let_assert!(
                    Ok(Some(document)) = collection.find_one(doc! { "_id": id }, None).await
                );
                assert_eq!(document, doc! { "_id": id, "hello": "world" });
                assert!(let Ok(()) = mongo.kill_and_clean().await);
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok());
    }
}

/// Seeds document into database, retrieves the seeded documents and
/// tests if the collected documents resemble the input documents
#[cfg_attr(feature = "tokio-runtime", tokio::test)]
#[cfg_attr(feature = "async-std-runtime", async_std::test)]
async fn seeding_document() {
    let documents = vec![
        mongodb::bson::doc! {"name": "Alice", "age": 30},
        mongodb::bson::doc! {"name": "Bob", "age": 25},
    ];

    // Create a new mongo instance and assert it's created successfully
    let mongo = TempMongo::new()
        .await
        .expect("Failed to create TempMongo instance");

    // Prepare seed document
    let prepared_seed_data = mongo.prepare_seed_document("test_3", "trex", documents.clone());

    match mongo.load_document(&prepared_seed_data).await {
        Ok(_) => println!("Data seeded successfully."),
        Err(e) => println!("Error seeding data: {:?}", e),
    }

    // Fetch documents from the database and compare
    let collection: mongodb::Collection<mongodb::bson::Document> =
        mongo.client().database("test_3").collection("trex");
    let mut cursor = collection
        .find(None, None)
        .await
        .expect("Failed to execute find command");

    // Collect documents from cursor
    let mut fetched_documents = Vec::new();
    while let Some(doc) = cursor
        .try_next()
        .await
        .expect("Failed during cursor traversal")
    {
        fetched_documents.push(doc);
    }

    // Remove '_id' field from the fetched documents
    for doc in &mut fetched_documents {
        doc.remove("_id");
    }

    // Assert that the fetched documents match what was seeded
    assert_eq!(
        documents, fetched_documents,
        "The seeded documents do not match the fetched documents"
    );

    assert!(let Ok(()) = mongo.kill_and_clean().await);
}

async fn test_temp_mongo_docker() {
    // Initialize TempMongoDocker
    let mut temp_mongo_docker = TempMongoDocker::new().expect("Failed to create TempMongoDocker");

    // Create a MongoDB container
    let_assert!(Ok(_container) = temp_mongo_docker.create().await);

    // Assuming `create` also initializes `mongo_client`
    let mongo_client = temp_mongo_docker
        .mongo_client
        .as_ref()
    let database = mongo_client.database("test");
        .expect("MongoDB client not initialized");
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
    let mut temp_mongo_docker = TempMongoDocker::new().expect("Failed to create TempMongoDocker");
    // Create a TempMongoDocker instance

    // Set up the environment (assuming this creates a container named "temp_mongo_docker")
    temp_mongo_docker
        .await
        .expect("Failed to create environment");
        .create()

    // Test the container_status function
    let status = temp_mongo_docker.container_status().await;
    match status {
        Ok(status) => assert!(status,"The container is found!"),
        Err(error) => panic!("Error while checking container status: {:?}", error),
    }
}
