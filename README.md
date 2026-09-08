# kata-lifecycle

A Rust-based lifecycle testing tool for Kata Containers.

The project provides a small end-to-end test harness for creating Kata containers, triggering lifecycle operations, and checking whether related resources are cleaned up correctly.

## Goals

The first version focuses on the following workflow:

```text
Create a Kata container
→ Trigger a lifecycle operation
→ Remove the containerd task and container
→ Check QEMU and Kata-related processes
→ Produce a structured test result
```

The initial target environment is:

- Fedora
- containerd
- runtime-rs
- QEMU
- KVM
- busybox

## Current Status

The project is under active development.

Currently implemented:

- `ctr` command execution with timeouts
- `ctr version`
- `ctr images pull`
- `ctr run` with the Kata runtime
- `ctr tasks list`
- `ctr tasks kill`
- `ctr tasks rm`
- `ctr containers rm`
- `sigkill` lifecycle scenario
- `/proc` process snapshots
- QEMU, Kata shim, and virtiofsd detection
- Process baseline comparison
- Process cleanup waiting with a timeout
- Structured leaked-process information
- JSON scenario reports
- Basic CLI commands

## Usage

Show the available commands:

```bash
cargo run -- --help
```

Run the smoke check:

```bash
cargo run -- smoke
```

Run the SIGKILL lifecycle scenario:

```bash
cargo run -- run sigkill
```

The SIGKILL scenario performs the following operations:

```text
ctr run --detach
→ ctr tasks kill --signal SIGKILL
→ ctr tasks rm
→ ctr containers rm
→ Check newly created QEMU, Kata shim, and virtiofsd processes
```

## Requirements

The real lifecycle scenarios require:

- Rust and Cargo
- `ctr`
- containerd
- Kata Containers
- QEMU
- KVM
- A locally available busybox image
- Permission to inspect `/proc`

The tests that use fake command runners and temporary procfs fixtures do not require a running containerd or Kata environment.

## Development

Run the complete test suite:

```bash
cargo test
```

Run a specific test:

```bash
cargo test <test_name>
```

Run the formatter:

```bash
cargo fmt
```

Run Clippy:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## Design

The project is organized around a few small interfaces:

```text
CommandRunner
    Executes external commands and captures their results

CtrClient
    Provides typed operations for ctr commands

Scenario
    Describes a container lifecycle workflow

ProcessCollector
    Reads process information from procfs

ScenarioReport
    Represents the result of a lifecycle test
```

The implementation uses test doubles for command execution so that lifecycle logic can be tested without requiring a real Kata installation.

## Test-Driven Development

The project follows a test-first development workflow:

```text
RED → GREEN → REFACTOR
```

Each feature is implemented as a vertical slice:

1. Add one behavior-focused test.
2. Implement the smallest change that makes it pass.
3. Refactor only after the test is green.
4. Run the complete test suite.

## Roadmap

Planned improvements include:

- `normal-exit` scenario
- `sigterm` scenario
- `failed-start` scenario
- Better cleanup diagnostics
- JUnit XML reports
- Unique run IDs and artifact directories
- Serial stress testing
- Parallel stress testing
- cgroup checks
- mount checks
- network checks
- `/dev/shm` checks
- CI integration
- Reproducible Kata issue reports

## Explicitly Out of Scope for the First Version

The first version does not target:

- Kubernetes
- Dragonball
- Firecracker
- GPU workloads
- Direct containerd API integration
- A complete resource accounting model

## License

License information will be added before the first public release.
