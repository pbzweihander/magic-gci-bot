# magic-gci-bot

_magic-gci-bot_ or _MagicBot_ is an AI GCI/AWACS bot for DCS utilizing SimpleRadioStandalone, Tacview realtime telemetry, and OpenAI platform.

__Currently work in progress and more like a proof of concept__

## Features

- Bogey Dope

### TODOs

- Tripwire (warning radius)
- Vector to homeplate

## DCS Server Requirements

- SimpleRadioStandalone server
- Tacview realtime telemetry server

## Usage

Copy `config.example.toml` to `config.toml` and edit it. You need an OpenAI platform API key.

```
cargo run -- --config config.toml
```

## License

[MIT License](./LICENSE)
