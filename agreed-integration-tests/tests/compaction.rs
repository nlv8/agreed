mod fixtures;

use std::sync::Arc;

use agreed::raft::MembershipConfig;
use agreed::{Config, SnapshotPolicy};
use anyhow::Result;
use maplit::hashset;

use fixtures::{sleep_for_a_sec, RaftRouter};
use tracing::info;

const ENTRIES_BETWEEN_SNAPSHOTS_LIMIT: u64 = 500;
const ORIGINAL_LEADER: u64 = 0;
const ADDED_FOLLOWER: u64 = 1;
const CLIENT_ID: &str = "client";

/// Compaction test.
///
/// Test plan:
///
///   1. Create a single-node cluster of Node 0.
///   1. Send enough requests to the node that log compaction will be triggered.
///   1. Add a new node (Node 0), and assert that it received the snapshot.
///
/// RUST_LOG=agreed,memstore,compaction=trace cargo test -p agreed --test compaction
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn compaction() -> Result<()> {
    fixtures::init_tracing();

    let router = {
        info!("--- Setup test dependencies");

        let config = Arc::new(
            Config::build("test".into())
                .snapshot_policy(SnapshotPolicy::LogsSinceLast(
                    ENTRIES_BETWEEN_SNAPSHOTS_LIMIT,
                ))
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
        info!("--- Triggering compaction");

        // Send enough requests to the cluster that compaction on the node should be triggered.
        // On pristine startup, we always put a single entry into the log. Thus, adding
        // LIMIT - 1 entries more to the log exactly triggers compaction.
        router
            .client_request_many(
                ORIGINAL_LEADER,
                CLIENT_ID,
                (ENTRIES_BETWEEN_SNAPSHOTS_LIMIT - 1) as usize,
            )
            .await;
    }

    // Wait to ensure there is enough time for a snapshot to be built.
    sleep_for_a_sec().await;

    {
        info!("--- Asserting the creation of the snapshot");

        router
            .assert_stable_cluster(Some(1), Some(ENTRIES_BETWEEN_SNAPSHOTS_LIMIT))
            .await;
        router
            .assert_storage_state(
                1,
                ENTRIES_BETWEEN_SNAPSHOTS_LIMIT,
                Some(0),
                ENTRIES_BETWEEN_SNAPSHOTS_LIMIT,
                Some((
                    ENTRIES_BETWEEN_SNAPSHOTS_LIMIT.into(),
                    1,
                    MembershipConfig {
                        members: hashset![0],
                    },
                )),
            )
            .await;
    }

    sleep_for_a_sec().await;

    {
        info!("--- Adding new node to the cluster");

        router.new_raft_node(ADDED_FOLLOWER).await;
        router
            .add_non_voter(ORIGINAL_LEADER, ADDED_FOLLOWER)
            .await
            .expect("failed to add new node as non-voter");

        sleep_for_a_sec().await;

        let _ = router.add_voter(ORIGINAL_LEADER, ADDED_FOLLOWER).await;
    }

    sleep_for_a_sec().await;

    {
        info!("--- Asserting whether the follower received the snapshot");

        // +1 because of the config change
        router
            .assert_stable_cluster(Some(1), Some(ENTRIES_BETWEEN_SNAPSHOTS_LIMIT + 1))
            .await;

        let expected_snapshot = Some((
            ENTRIES_BETWEEN_SNAPSHOTS_LIMIT.into(),
            1,
            MembershipConfig {
                members: hashset![0u64],
            },
        ));

        router
            .assert_storage_state(
                1,
                ENTRIES_BETWEEN_SNAPSHOTS_LIMIT + 1,
                None, // This value is None because non-voters do not vote.
                ENTRIES_BETWEEN_SNAPSHOTS_LIMIT, // no +1 because the additional entry was a config change
                expected_snapshot,
            )
            .await;
    }

    Ok(())
}
