# TL;DR: Log everything (except X11)

```
RUST_LOG=debug WAYLAND_DEBUG=1 cargo run --example keyboard
```

# Debugging
In order to better debug issues, the following steps can be taken:

## Turn on logging
I use the `env_logger` crate in the examples. Run them with the environment variable `RUST_LOG=debug` to turn on logging. If you don't want to see all log messages, you can turn them on for only a specific module (`RUST_LOG=enigo::platform::x11=debug`). For more examples, have a look at the [official documentation](https://docs.rs/env_logger/latest/env_logger/).

If you want to debug your own application, run your executable with enabled logging. `enigo` uses the log crate, so you have to uses one of [these crates](https://docs.rs/log/latest/log/#available-logging-implementations) to see its output. 

Full command to turn on debug messages for the keyboard example when using only the x11rb feature:

```
RUST_LOG=enigo::platform::x11=debug cargo run --example keyboard --features x11rb --no-default-features
```

## Linux

On Linux, we can additionally print the messages that are exchanged with the compositor. It depends on the used protocol how to turn this on.

### Wayland
Run the executable with the environment variable `WAYLAND_DEBUG=1`.

### X11
In order to see the messages, a proxy server for the X11 messages needs to be used. You can use `xtrace` for that, but I usually use the xtrace-example from the x11rb crate instead. Here are the steps to get that running:

```
git clone https://github.com/psychon/x11rb.git
cd x11rb/xtrace-example
cargo run
# The output of the command will tell you how to use it.
# Example:
# /home/pentamassiv/x11rb/target/debug/xtrace-example cargo run --example keyboard --features x11rb
```

## Log everything (including X11)

```
RUST_LOG=debug WAYLAND_DEBUG=1 /home/pentamassiv/x11rb/target/debug/xtrace-example cargo run --example keyboard --features x11rb,wayland --no-default-features
```