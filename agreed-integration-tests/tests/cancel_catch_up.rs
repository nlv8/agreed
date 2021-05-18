mod fixtures;

use std::sync::Arc;

use agreed::Config;
use anyhow::Result;

use fixtures::{sleep_for_a_sec, RaftRouter};
use tracing::info;

const ORIGINAL_LEADER: u64 = 0;
const NODE_TO_ADD: u64 = 1;
const CLIENT_ID: &str = "client";
const CLUSTER_NAME: &str = "test";
const BULK_REQUEST_ENTRY_COUNT: u64 = 5000;

/// Cancel catch-up test.
///
/// Test plan:
///
///   1. Create a single-node cluster of Node 0.
///   1. Add a new nodes 1, 2, 3, 4 and assert that they've joined the cluster properly.
///   1. Propose a new config change where the old leader, Node 0 is not present, and assert that it steps down.
///   1. Temporarily isolate the new leader, and assert that an even newer leader takes over.
///   1. Restore the isolated node and assert that it becomes a follower.
///
/// RUST_LOG=agreed,memstore,dynamic_membership=trace cargo test -p agreed --test dynamic_membership
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn cancel_catch_up() -> Result<()> {
    fixtures::init_tracing();

    let router = {
        info!("--- Setup test dependencies");

        let config = Arc::new(
            Config::build(CLUSTER_NAME.into())
                .validate()
                .expect("failed to build Raft config"),
        );
        Arc::new(RaftRouter::new(config))
    };

    {
        info!("--- Initializing and asserting on a single-node cluster");

        router.new_raft_node(ORIGINAL_LEADER).await;
        sleep_for_a_sec().await;
        router.assert_pristine_cluster().await;

        router.initialize_from_single_node(ORIGINAL_LEADER).await?;
        sleep_for_a_sec().await;
        router.assert_stable_cluster(Some(1), Some(1)).await;
    }

    {
        info!("--- Add few entries");

        router
            .client_request_many(ORIGINAL_LEADER, CLIENT_ID, BULK_REQUEST_ENTRY_COUNT as usize)
            .await;
        sleep_for_a_sec().await;
        router.assert_stable_cluster(Some(1), Some(BULK_REQUEST_ENTRY_COUNT + 1)).await;
    }

    let add_voter_result = {
        info!("--- Attempting to add the node, knowing it's not even connected yet");

        router.new_raft_node(NODE_TO_ADD).await;
        sleep_for_a_sec().await;
        router.isolate_node(NODE_TO_ADD).await;
        sleep_for_a_sec().await;
        router.add_voter(ORIGINAL_LEADER, NODE_TO_ADD).await
    };

    {
        info!("--- Asserting that we received an error");

        assert!(add_voter_result.is_err());
    }

    {
        info!("--- Actually adding the new node");

        router.restore_node(NODE_TO_ADD).await;
        let _ = router.add_voter(ORIGINAL_LEADER, NODE_TO_ADD).await;
    }

    sleep_for_a_sec().await;

    {
        info!("--- Asserting on the new cluster configuration");

        router.assert_stable_cluster(Some(1), Some(BULK_REQUEST_ENTRY_COUNT + 2)).await;
    }

    Ok(())
}
