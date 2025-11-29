use bollard::Docker;
use deadpool::managed::Pool;
use worker::{executer::exec_testcase, pool::ContainerGroup};

#[tokio::test]
async fn test_testcase_output() {
    let docker = Docker::connect_with_defaults().unwrap();
    let manager = ContainerGroup::new(docker.clone(), "python:3.12-slim")
        .await
        .unwrap();
    let docker_pool: Pool<ContainerGroup> = Pool::builder(manager).max_size(3).build().unwrap();

    let container = docker_pool.get().await.unwrap();

    let code = "import sys\n\ndata = sys.stdin.read().strip().split()\ndata = list(map(int, data))\n\nT = data[0]\nnums = data[1:1+T]\n\nfor n in nums:\n    if n % 2 == 0:\n        print(\"EVEN\")\n    else:\n        print(\"ODD\")";
    let testcase = "3 1 5 2";
    let command = "python -c";
    let expected_output = "ODD\nODD\nEVEN\n";

    let output = exec_testcase(docker, &container.id, code, testcase, command)
        .await
        .unwrap();

    assert_eq!(output, expected_output);

    docker_pool.manager().close().await;
}
