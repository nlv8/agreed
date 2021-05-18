mod fixtures;

use std::sync::Arc;

use agreed::{Config, State};
use anyhow::Result;
use maplit::hashset;
use tracing::info;

use fixtures::{sleep_for_a_sec, RaftRouter};

const CLUSTER_NAME: &str = "test";

/// Leader stepdown test.
///
/// Test plan:
///
///   1. Create a single-node cluster of Node 0.
///   1. Add three new nodes (1, 2, 3) as Voters.
///   1. Ask the leader (Node 0) to remove itself from the cluster.
///   1. Ensure that the old leader (Node 0) no longer gets updates.
///
/// RUST_LOG=agreed,memstore,stepdown=trace cargo test -p agreed --test stepdown
#[tokio::test(flavor = "multi_thread", worker_threads = 5)]
async fn stepdown() -> Result<()> {
    fixtures::init_tracing();

    // Setup test dependencies.
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

        router.new_raft_node(0).await;
        sleep_for_a_sec().await;
        router.assert_pristine_cluster().await;

        router.initialize_from_single_node(0).await?;
        sleep_for_a_sec().await;
        router.assert_stable_cluster(Some(1), Some(1)).await;
    }

    let original_leader = {
        info!("--- Adding nodes 1, 2, 3 to the cluster");

        let original_leader = router
            .leader()
            .await
            .expect("expected the cluster to have a leader");
        assert_eq!(0, original_leader, "expected original leader to be node 0");
        for node in 1u64..=3 {
            info!("--- Adding node {}", node);
            router.new_raft_node(node).await;
            let _ = router.add_voter(original_leader, node).await;
        }

        original_leader
    };

    sleep_for_a_sec().await;

    {
        info!("--- Asserting whether the cluster formed properly");

        let metrics = router
            .latest_metrics()
            .await
            .into_iter()
            .find(|node| node.id == original_leader)
            .expect("expected to find metrics on original leader node");
        let cfg = metrics.membership_config;
        assert_eq!(
            metrics.state,
            State::Leader,
            "expected node 0 to be the old leader"
        );
        assert_eq!(
            metrics.current_term, 1,
            "expected old leader to still be in first term, got {}",
            metrics.current_term
        );
        // 7 because
        //   1 - initial entry
        //   2 - add node 1
        //   3 - add node 2
        //   4 - add node 3
        assert_eq!(
            metrics.last_log_index, 4,
            "expected old leader to have last log index of 4, got {}",
            metrics.last_log_index
        );
        assert_eq!(
            metrics.last_applied, 4,
            "expected old leader to have last applied of 4, got {}",
            metrics.last_applied
        );
        assert_eq!(
            cfg.members,
            hashset![0, 1, 2, 3],
            "expected old leader to have membership of [0, 1, 2, 3], got {:?}",
            cfg.members
        );
    }

    {
        info!("--- Old leader stepping down");

        let _ = router.remove_voter(original_leader, original_leader).await;
    }

    sleep_for_a_sec().await;

    {
        info!("--- Asserting cluster state after the old leader stepped down");

        let metrics = router
            .latest_metrics()
            .await
            .into_iter()
            .find(|m| m.state == State::Leader)
            .expect("expected the cluster to have a new leader");

        let cfg = metrics.membership_config;
        assert_eq!(
            metrics.current_term, 2,
            "expected the new leader to be in term 2, got {}",
            metrics.current_term
        );
        // 10 because
        //   we carried over 4 from before
        //   5 - node removal
        //   6 - the new leader starts its term with a new entry
        assert_eq!(
            metrics.last_log_index, 6,
            "expected the new leader to have last log index of 6, got {}",
            metrics.last_log_index
        );
        assert_eq!(
            metrics.last_applied, 6,
            "expected the new leader to have last applied of 6, got {}",
            metrics.last_applied
        );
        assert_eq!(
            cfg.members,
            hashset![1, 2, 3],
            "expected new cluster to have membership of [1, 2, 3], got {:?}",
            cfg.members
        );
    }

    {
        info!("--- Asserting that the stepped down leader no longer gets updates");

        let new_leader_metrics = router
            .latest_metrics()
            .await
            .into_iter()
            .find(|m| m.state == State::Leader)
            .expect("expected the cluster to have a new leader");

        let old_leader_metrics = router
            .latest_metrics()
            .await
            .into_iter()
            .find(|m| m.id == original_leader)
            .expect("expected the cluster to have a new leader");

        assert!(
            old_leader_metrics.current_term < new_leader_metrics.current_term,
            "expected the old leader to have term less than that of the new leader"
        );

        assert!(
            old_leader_metrics.last_log_index < new_leader_metrics.last_log_index,
            "expected the old leader to have last log index less than that of the new leader"
        );
    }

    Ok(())
}
