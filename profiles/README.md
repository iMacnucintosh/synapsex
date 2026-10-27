# Onboard profile snapshots

JSON dumps of Basilisk V4 Pro onboard slots (DPI, wheel, lighting, L0/L1 remaps).

## Hyperland (slot 1)

```sh
# restore onto the slot stored in the file (normally 1)
cargo run -p synapsex-cli --release -- profile import profiles/hyperland.json

# or force a destination slot
cargo run -p synapsex-cli --release -- profile import profiles/hyperland.json --slot 1
```

Re-export after a known-good config:

```sh
cargo run -p synapsex-cli --release -- profile export --slot 1 -o profiles/hyperland.json
```
