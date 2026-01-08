# antithesis_browser

A prototype of generative browser testing.

## Usage

### Running tests

```bash
cargo run -- test https://example.com
```

Run headless:

```bash
cargo run -- test https://example.com --headless
```

See debug logs:

```bash
RUST_LOG=antithesis_browser=debug cargo run -- test https://example.com --headless
```

## Development

### Integration tests

```bash
cargo test --test integration_tests
```

### Changing dependencies

After any changes to dependencies in Cargo.toml:

```bash
crate2nix generate -o nix/Cargo.nix
```

