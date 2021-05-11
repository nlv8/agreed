<h1 align="center">Agreed</h1>
<div align="center">
    <strong>
        Fork of <a href="https://github.com/async-raft/async-raft">async-raft</a>, the <a href="https://tokio.rs/">Tokio</a>-based Rust implementation of the <a href="https://raft.github.io/">Raft distributed consensus protocol</a>.
    </strong>
</div>
<br/>
<div align="center">

[![Continuous Integration](https://github.com/nlv8/agreed/actions/workflows/continuous-integration.yaml/badge.svg)](https://github.com/nlv8/agreed/actions/workflows/continuous-integration.yaml)
[![Crates.io](https://img.shields.io/crates/v/agreed.svg)](https://crates.io/crates/agreed)
[![docs.rs](https://docs.rs/agreed/badge.svg)](https://docs.rs/agreed)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-MIT)

</div>
<br/>

Agreed is an implementation of the [Raft](https://raft.github.io) consensus protocol in Rust, intented to serve as a basis for distributed data systems.

**Getting Started**

If you want to get started with building applications on top of Agreed, then checkout out [The Agreed Guide](https://nlv8.github.io/agreed). Afterwards, feel free to jump into [the documentation](https://docs.rs/agreed/latest/agreed/).

## Features

  * Fully asynchronous, built on top of [Tokio](https://tokio.rs/).
  * Pluggable storage and network layer.
  * Log compaction with snapshots and snapshot streaming.
  * Fully pipelined and batched log replication with congestion control.
  * Single-node cluster membership change operations.
  * Non-Voter nodes for data replication/change data capture.
  * Instrumented with [tracing](https://docs.rs/tracing/).

## Original Author

This project, including the guide, was originally written by [Anthony Dodd](https://github.com/thedodd) as [async-raft](https://github.com/async-raft/async-raft). Huge props to him! :rocket:

## License

Agreed is licensed under the terms of the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at your choosing.

----

**NOTE:** the appearance of the "section" symbols `§` throughout this project are references to specific sections of the Raft spec.
