# Patchify

The Patchify crate is an auto-update library, providing the ability for Rust
applications to automatically update themselves.

It includes functionality to embed into both the application itself and into a
web-based API server, to give the application the ability to check for updates,
and for the server to fully handle those updates. The library modules are
extremely easy to use, with simple configuration and minimal code required to
get up and running.

The application to equip with auto-update powers can potentially be anything
that runs persistently: a web server, a desktop application, a command-line
tool, etc.

There is a [roadmap for the project](ROADMAP.md), which sets out the planned
releases and their associated functionality, and indicates current status
according to the intended goals.

The main sections in this README are:

  - [Features](#features)
  - [Modules](#modules)
  - [Setup](#setup)
  - [End-to-end example](#end-to-end-example)


## Features

[Axum]:    https://crates.io/crates/axum
[Figment]: https://crates.io/crates/figment
[Hyper]:   https://crates.io/crates/hyper
[Tracing]: https://crates.io/crates/tracing

The main high-level points of note are:

  - Client application
      - Fully-autonomous update checking and upgrade process
      - Configurable checking intervals
      - Automatic application restart
      - Ability to register and manage critical actions to choreograph upgrades
      - Update status broadcaster for application-wide status updates
      - Verification of release files using SHA256 hashes
      - Verification of HTTP response signatures using public keys
  - API server
      - Webserver-agnostic, but with full integration for [Axum][]
      - Logging of HTTP requests and events using [Tokio Tracing][Tracing]
      - Streaming of large release files for memory efficiency
      - Signing of HTTP responses using private keys
  - Full yet minimal examples working out of the box
      - Configuration from config files and env vars using [Figment][]
      - High-performance asynchronous HTTP server using [Tokio Hyper][Hyper]
      - Based on the robust and ergonomic web framework [Axum][]
  - Fully-documented codebase
  - Full test coverage with unit and integration tests

### Signing and verification

HTTP responses will be signed with the server's private key, allowing connecting
clients to verify that they have not been tampered with. The key format used is
Ed25519, which is faster and more secure than RSA.

### Hashing and verification

Release files are checked against SHA256 hashes when the server starts up, and
the client verifies the hashes of the files it downloads. This ensures that the
files are an accurate replica of the original, and have not been tampered with.

### Streaming

The server will stream large release files to clients, which is more efficient
than reading the entire file into memory before sending it. This is
fully-configurable.


## Modules

Currently, the following modules are provided:

  - [`client`](#client)
  - [`server`](#server)

The modules are designed to be used independently.

### client

The [`client`](https://docs.rs/patchify/latest/patchify/client/index.html)
module provides functionality to embed into an application, to give it
auto-update abilities.

#### Example

Use of the client module is essentially as follows:

  1. Bring in the `patchify::client` module.
  2. Create a new `Updater` instance, passing in the appropriate configuration
     via a `Config` instance.

That's it! The `Updater` instance will spawn threads to check for updates in the
background, at the specified intervals. It will also log its activity, and you
can listen in to that activity from inside your application using the provided
`Updater.subscribe()` method.

An example of this in use is provided as `examples/cli-app-*.rs`. There are two
versions, so that the upgrade capabilities can be seen in action.

To run the examples, use the following commands:

```sh
cargo run --example cli-app-v1
cargo run --example cli-app-v2
```

The following points are worth noting:

  - The `tokio::signal::ctrl_c()` function is used to wait for a `Ctrl-C`
    signal, to keep the application running so that the update process can be
    observed.
  - The example allows configuration of the application name, the location of
    the API server, the API server's public key, and the update interval.

This is covered in more depth in the [end-to-end example](#end-to-end-example)
guide.

### server

The [`server`](https://docs.rs/patchify/latest/patchify/server/index.html)
module provides functionality to embed into a web-based API server, to give it
the ability to serve updates and related information to applications. As well as
the critical functionality, it also provides endpoint implementations
suitable for use with [Axum][].

#### Example

A fairly simple, yet comprehensive example of how to use the server module is
provided as `examples/axum-server.rs`. This example is essentially an expanded
version of the `testbins/standard-api-server` integration test, and relies upon
the same common code used by all the integration tests, but adds the ability to
accept configuration. It is a good starting point for a real-world server
implementation.

To run the example, use the following command:

```sh
cargo run --example axum-server
```

The following points are worth noting:

  - The full implementation logic can be found in `tests/common/server.rs`,
    which is used by the integration tests. This provides the `initialize()` and
    `create_server()` functions.
  - The `tokio::signal::ctrl_c()` function is used to wait for a `Ctrl-C`
    signal, which will help the server to shut down cleanly. This is not
    strictly required, but is a common pattern, and good practice. It is used in
    the integration tests to ensure that the test server continues running in
    the background while it is still needed, and can then be told to shut down
    when the tests are complete.
  - The example allows configuration of the application name, the host and port
    to run on, the folder containing the release files, and a list of versions
    with their associated hashes. In order to use a randomly-assigned port, set
    the value to `0`.

By default there will be no versions configured for the example. To add some,
release files should be created in the configured releases folder, named
appropriately, and the hashes should be calculated and added to the list. The
example will then serve these files and hashes to clients that request them.
This is covered in more depth in the [end-to-end example](#end-to-end-example).


## Setup

[Rustup]: https://rustup.rs/

The steps to set up a project using Patchify are simple and standard. You need a
reasonably-recent Rust environment, on a Linux machine. There are currently no
special requirements beyond what is needed to build a standard Rust project.

### Environment

There are some key points to note about the environment you choose:

  - Debian and Ubuntu are the Linux distros of choice, although other distros
    should also work just fine, as there are no special requirements.
  - Running natively on Windows is not currently targeted or tested, but there
    are plans to support it properly in future. Running on WSL does work fine.
  - Running natively on MacOS is untested, although there is no known technical
    reason why it would not work.

### Configuration

Patchify is configured using `Config` structs that are passed in to the
`Updater` instance on the client side, and to the `Core` instance on the server
side. These are documented here:

  - [`client::Config`](https://docs.rs/patchify/latest/patchify/client/struct.Config.html)
  - [`server::Config`](https://docs.rs/patchify/latest/patchify/server/struct.Config.html)

### Integration

For the client, the creation of an `Updater` instance with a suitable `Config`
is all that is required. For the server, in addition to creating a `Core`
instance, there is also a choice about whether to integrate with [Axum][] or
call the `Core` methods directly from your own endpoint functions.

Notably, the client functionality provides two main touchpoints to help
orchestrate the upgrade process: the `Updater.subscribe()` method, and the
critical actions counter.

#### Critical actions counter

The critical actions counter is a simple counter that can be incremented and
decremented, and is used to establish when it's safe to restart the application
after an upgrade. It is a simple way to ensure that the application is not in
the middle of a critical operation when it is restarted.

The basic premise is that if an application is about to do something that
should not be interrupted, the counter can be incremented, and then decremented
when the operation is complete. The `Updater` instance will then only restart
the application when the counter is zero. If the updater is about to restart the
application, then it will deny the start of any new critical actions until the
restart has completed.

This makes it very easy to integrate the upgrade process into an application
with confidence around when exactly a restart will occur.

If more control is needed over the manner in which the restart occurs, then it
is advisable to register a critical action when the application starts, and
never deregister it, instead relying upon the [status change events](#status-event-subscription)
to detect when a restart is needed, and handle it in a customised way.

#### Status event subscription

The `Updater.subscribe()` method allows the application to listen in to the
activity of the `Updater` instance, and to react to the status changes that it
broadcasts. This is useful for updating the application's UI, or for logging
purposes. It also provides a way of manually controlling the upgrade process, if
necessary.


## End-to-end example

It's not possible to have a 100%-working end-to-end example available right out
of the box, because the release files need to be generated and registered with
the server. However, all of the ingredients are provided, and by following the
few short steps in this section you can have a fully-working example up and
running in no time.

### Prerequisites

You will need to have a Rust environment set up, on Linux, along with a clone of
the Patchify repository. The steps in this example assume that commands are
being run from the root of the repository. This is for demonstration purposes
only, and in a real-world scenario you would be working with your own
application repository and including Patchify as a dependency, using it as
described in the rest of this README.

### 1. Prepare release directory

Assuming a fresh clone, create a new directory for the server to serve releases
from:

```sh
mkdir -p /tmp/patchify-releases
```

### 2. Build application releases

We now need to compile the client examples. Although you can run them directly,
they also get compiled when the tests are run, so that's the simplest way to
build them, and also ensures that all the tests are passing in your environment.

```sh
cargo test
```

### 3. Copy release files

Once the tests have completed, you need to copy the compiled binaries to the
releases directory:

```sh
cp target/debug/examples/cli-app-v1 /tmp/patchify-releases/cli-app-1.0.0
cp target/debug/examples/cli-app-v2 /tmp/patchify-releases/cli-app-2.0.0
```

Note the change in filename. This is because Cargo wants each example to have a
different crate, and what we want to do here is actually create two different
versions of the same application. Therefore the examples are named `v1` and
`v2`, corresponding to versions `1.0.0` and `2.0.0` respectively.

In order to be able to run the client application later, copy the application
binary to your current directory:

```sh
cp target/debug/examples/cli-app-v1 ./cli-app
```

Note again the change in filename. This is because we don't care what version
the application is, we just want to run it. The file we copied will be replaced
with the new version when the updater runs.

*Note that if your local Cargo is set up to use a different directory for
builds, you will need to adjust the paths accordingly.*

### 4. Configure API server

Now we need to run the server example. This will serve the release files that we
just created, and will also provide the API endpoints for the client application
to interact with.

But first, we need some configuration. Copy the `examples/axum-server.toml` file
to your current working directory:

```sh
cp examples/axum-server.toml .
```

Now edit it and set the `releases` value to `/tmp/patchify-releases`, and add
the two versions in, with their associated hashes. The file should look
something like this:

```toml
appname  = "cli-app"
host     = "127.0.0.1"
port     = 8000
releases = "/tmp/patchify-releases"

[versions]
"1.0.0" = "beef1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f"
"2.0.0" = "cafe1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f"
```

In order to obtain the hashes, you can use the `sha256sum` command:

```sh
sha256sum /tmp/patchify-releases/cli-app-1.0.0
sha256sum /tmp/patchify-releases/cli-app-2.0.0
```

By default the server will run on port `8000`, and will serve from localhost.
Feel free to change these values to suit your environment.

### 5. Run API server

Now you can run the server example. Note that you will need to keep this open in
a different terminal window, so from this point on you will have two terminal
windows open.

```sh
cargo run --example axum-server
```

The server will check the validity of the release files when it starts up, and
exit if there are any errors. Note that this may take a few seconds, as it needs
to read the entirety of each file to calculate the hash.

### 6. Configure client application

Copy the `examples/cli-app.toml` file to your current working directory:

```sh
cp examples/cli-app.toml .
```

Now edit it and set the `updater_api_server` value to whatever the server is
running on. This should be as you configured, but the server will print out the
address when it starts up, along with the public key. You will need to add that
key to the client configuration as well, as `updater_api_key`:

```toml
appname            = "cli-app"
updater_api_server = "http://127.0.0.1:8000/api/"
updater_api_key    = "beef1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f"
update_on_startup  = false
update_interval    = 10
```

The example server generates a new key each time it starts up, so you will need
to copy it from the server's output each time you restart the server. This is to
make the process more robust for demonstration purposes than accepting a private
key in the server configuration, in case it is wrongly generated, which can
cause frustration.

Note that in the configuration example above, the `update_on_startup` value is
set to `false`. This is because we want to see the update process in action, so
we don't want the application to update itself when it starts up. Additionally,
the `update_interval` is set to `10`, so that the application will check for
updates every 10 seconds, which is not too long to wait. Feel free to experiment
with these values and observe the differences in behaviour.

### 7. Run client application

We're now ready to run the client application!

```sh
./cli-app
```

The application will start up and check for updates at the interval you
specified in the configuration. If there are updates available, it will download
them, verify them, install them, and restart itself. You should see the status
change events being printed to the console, and you should also see the printed
version number change when the application restarts.


