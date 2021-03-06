mod fixtures;

use std::sync::Arc;

use agreed::Config;
use anyhow::Result;

use fixtures::{sleep_for_a_sec, RaftRouter};
use tracing::info;

const ORIGINAL_LEADER: u64 = 0;
const CLUSTER_NAME: &str = "test";

/// Dynamic membership test.
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
async fn dynamic_membership() -> Result<()> {
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
        info!("--- Adding nodes 1, 2, 3, 4 as voters");

        for node in 1u64..=4 {
            info!("--- Adding node {}", node);
            router.new_raft_node(node).await;
            let _ = router.add_voter(ORIGINAL_LEADER, node).await;
        }
    }

    sleep_for_a_sec().await;

    {
        info!("--- Asserting on the new cluster configuration");

        router.assert_stable_cluster(Some(1), Some(5)).await;
    }

    {
        info!("--- Isolating original leader Node 0");

        router.isolate_node(ORIGINAL_LEADER).await;
    }

    // Wait for election and for everything to stabilize (this is way longer than needed).
    sleep_for_a_sec().await;

    let new_leader = {
        info!("--- Asserting that a new leader took over");

        router.assert_stable_cluster(Some(2), Some(6)).await;
        let new_leader = router.leader().await.expect("expected new leader");
        assert_ne!(
            new_leader, ORIGINAL_LEADER,
            "expected new leader to be different from the old leader"
        );

        new_leader
    };

    {
        info!("--- Restoring isolated old leader Node 0");

        router.restore_node(ORIGINAL_LEADER).await;
    }

    sleep_for_a_sec().await;

    {
        info!("--- Asserting that the leader of the cluster stayed the same");

        // We should still be in term 2, as leaders should not be deposed when
        // they are not missing heartbeats.
        router.assert_stable_cluster(Some(2), Some(6)).await;
        let current_leader = router
            .leader()
            .await
            .expect("expected to find current leader");
        assert_eq!(
            new_leader, current_leader,
            "expected cluster leadership to stay the same"
        );
    }

    Ok(())
}
