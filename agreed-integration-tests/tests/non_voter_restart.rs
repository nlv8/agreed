mod fixtures;

use std::sync::Arc;

use agreed::{Config, NodeId, Raft, State};
use anyhow::Result;
use fixtures::{sleep_for_a_sec, RaftRouter};
use tracing::info;

use crate::fixtures::MemRaft;

const LEADER: u64 = 0;
const NON_VOTER: u64 = 1;
const CLUSTER_NAME: &str = "test";
const CLIENT_ID: &str = "client";

/// Non Voter Restart test.
///
/// Test Plan
///
/// - brings 2 nodes online: one leader and one non-voter.
/// - write one log to the leader.
/// - asserts that the leader was able to successfully commit its initial payload and that the
///   non-voter has successfully replicated the payload.
/// - shutdown all and retstart the non-voter node.
/// - asserts the non-voter stays in non-vtoer state.
///
/// RUST_LOG=async_raft,memstore,non_voter_restart=trace cargo test -p async-raft --test
/// non_voter_restart
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn non_voter_restart() -> Result<()> {
    fixtures::init_tracing();

    let (router, config) = {
        info!("--- Setup test dependencies");

        let config = Arc::new(
            Config::build(CLUSTER_NAME.into())
                .validate()
                .expect("failed to build Raft config"),
        );
        let router = Arc::new(RaftRouter::new(Arc::clone(&config)));

        (router, config)
    };

    {
        info!("--- Initializing and asserting on a single-node cluster");

        router.new_raft_node(LEADER).await;
        sleep_for_a_sec().await;
        router.assert_pristine_cluster().await;

        router.initialize_from_single_node(LEADER).await?;
        sleep_for_a_sec().await;
        router.assert_stable_cluster(Some(1), Some(1)).await;
    }

    sleep_for_a_sec().await;

    {
        info!("--- Adding a Non-Voter to the cluster");

        router.new_raft_node(NON_VOTER).await;
        sleep_for_a_sec().await;
        let _ = router.add_non_voter(LEADER, NON_VOTER).await;
    }

    sleep_for_a_sec().await;

    {
        info!("--- Performing a single client request");

        router.client_request(LEADER, CLIENT_ID, 0).await;
    }

    {
        info!("--- Removing and shutting down the Leader");

        let (leader, _) = router.remove_node(LEADER).await.unwrap();
        assert_node_state(LEADER, &leader, 1, 2, State::Leader);
        leader.shutdown().await?;
    }

    sleep_for_a_sec().await;

    let non_voter_store = {
        info!("--- Removing and shutting down the Non-Voter");

        let (non_voter, non_voter_store) = router.remove_node(NON_VOTER).await.unwrap();
        assert_node_state(NON_VOTER, &non_voter, 1, 2, State::NonVoter);
        non_voter.shutdown().await?;

        non_voter_store
    };

    sleep_for_a_sec().await;

    {
        info!("--- Restarting and asserting the state of the Non-Voter");

        let restarted = Raft::new(NON_VOTER, config, router.clone(), non_voter_store);
        sleep_for_a_sec().await;
        assert_node_state(NON_VOTER, &restarted, 1, 2, State::NonVoter);
    }

    Ok(())
}

fn assert_node_state(
    id: NodeId,
    node: &MemRaft,
    expected_term: u64,
    expected_log: u64,
    state: State,
) {
    let m = node.metrics().borrow().clone();
    tracing::info!("node {} metrics: {:?}", id, m);

    assert_eq!(expected_term, m.current_term, "node {} term", id);
    assert_eq!(expected_log, m.last_log_index, "node {} last_log_index", id);
    assert_eq!(expected_log, m.last_applied, "node {} last_log_index", id);
    assert_eq!(state, m.state, "node {} state", id);
}
