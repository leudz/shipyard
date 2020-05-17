# Contributing to Shipyard

Thank you for taking interest in contributing to the Shipyard project.

This document intends to give you a quick tour of the tools we use to test and debug code.

If you feel that good advice is missing from this document, please feel free to make suggestions through [the Shipyard Zulip](https://shipyard.zulipchat.com/) or as [a new issue](https://github.com/leudz/shipyard/issues/new?title=Improve+the+Contributing+document&body=I%27d+like+to+suggest+we...).

## Communication

Many conversations start as raw ideas in [the Shipyard Zulip](https://shipyard.zulipchat.com/), since it's an easy medium for quick feedback.

Otherwise, you may open a new issue with a write-up of what you're trying to accomplish with Shipyard.

One of the hardest parts about determining which features to build into the core library and which features should be left for supporting libraries to build is a challenge of balancing the needs of many different projects. Sharing context for what you are trying to accomplish will ensure that we can collaborate towards the best approach that can benefit everyone.

## Testing

Currently, Shipyard uses [`cargo-make`](https://github.com/sagiegurari/cargo-make) alongside our [Makefile.toml](./Makefile.toml) to specify helpful tasks for testing different feature combinations.

```sh
# run all tests and static checks (miri may not work for macOS)
cargo make test

# test all feature combinations
cargo make test-all
```
