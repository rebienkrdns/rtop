use bollard::service::{ListNodesOptions, ListServicesOptions, ListTasksOptions};
use bollard::Docker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let docker = Docker::connect_with_local_defaults()?;

    // Check docker system info
    let info = docker.info().await?;
    println!("Swarm Node Info: {:?}", info.swarm);

    // Check list services
    let services = docker
        .list_services(None::<ListServicesOptions<String>>)
        .await?;
    println!("Services: {}", services.len());

    // Check list tasks
    let tasks = docker.list_tasks(None::<ListTasksOptions<String>>).await?;
    println!("Tasks: {}", tasks.len());

    // Check list nodes
    let nodes = docker.list_nodes(None::<ListNodesOptions<String>>).await?;
    println!("Nodes: {}", nodes.len());

    Ok(())
}
