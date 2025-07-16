# GitHub PR Comments to Markdown

## Why?

Written by and for Claude code, but can certainly be useful for humans as well.
You would think `gh` would support this, but it doesn't:
https://github.com/cli/cli/issues/359.

## Installation

Clone the repo, `cargo install --path .`, or `cargo install gh-pr-comments`.

### Examples

```bash
gh-pr-comments https://github.com/mozilla/mp4parse-rust/pull/435
cd /path/to/your/repo
gh-pr-comments 123
gh-pr-comments 456 --repo facebook/react
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
