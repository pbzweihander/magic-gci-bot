# magic-gci-bot

_magic-gci-bot_ is an AI GCI/AWACS bot for DCS utilizing SimpleRadioStandalone, OpenAI speech recognition, and OpenAI speech generation.
_MagicBot_ is a partial successor of OverlordBot.

__Currently work in progress and more like a proof of concept__

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
