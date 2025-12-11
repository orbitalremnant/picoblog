# picoblog

A minimalistic static site generator written in Rust.

![Promo](https://github.com/orbitalremnant/picoblog/blob/main/promo.gif?raw=true)

`picoblog` turns a directory of Markdown and text files into a single, self-contained `index.html` with built-in search[^1] and tag filtering. It's designed for speed, simplicity, and zero-dependency deployment.

[^1]: For security reasons, search only works when the site is being server through a server.

# Key Features

-   **Single-Page Output**: Generates one `index.html` for easy hosting.
-   **Client-Side Search**: Instant full-text search with a pre-built JSON index.
-   **Tag Filtering**: Dynamically generates tag buttons to filter posts.
-   **Flexible Content**: Supports YAML frontmatter and infers metadata from filenames.
-   **Automatic Favicons**: Creates favicons from your blog's title.
-   **Highly Portable**: A single, dependency-free binary.

# Quick Start

```sh
cargo install picoblog
```

```sh
picoblog \
    --title "My Awesome Blog" \
    --description "A blog about tech and adventures." \
    --share "X:https://twitter.com/intent/tweet?url={URL}&text={TITLE}"
```

**Done!** Your new blog is ready in the `public/` directory. Open `public/index.html` in your browser to see it.

# Content Format

Place your `.md` and `.txt` files in a source directory (or many).

**Markdown with Frontmatter (`.md`):**

```txt
---
title: "My Post Title"
description: "A short summary of the post."
tags: ["rust", "tech"]
---

The main content of the post goes here.
#hashtags are also automatically detected.
```

**Plain Text (`.txt`):**

The filename is used for the title and date (e.g., `2024-10-26-quick-note.txt`).

```txt
This is a plain text post.
Line breaks are preserved.

#text #notes
```

---

## Social Sharing Links

You can generate social media sharing links for each article using the `--share` flag. This flag can be used multiple times to define different sharing providers.

### Syntax

The format for the flag is:
`--share "PROVIDER_NAME:URL_TEMPLATE"`

-   **`PROVIDER_NAME`**: The text that will be displayed for the link (e.g., "X", "LinkedIn", "Reddit").
-   **`URL_TEMPLATE`**: The sharing URL provided by the social media platform.

### Placeholders

The `URL_TEMPLATE` can contain the following placeholders, which `picoblog` will automatically replace with the article's data. The values are URL-encoded for safety.

-   **`{URL}`**: Replaced by the article's `link_url` from the frontmatter or the first URL found in the post content.
-   **`{TITLE}`**: Replaced by the article's title.
-   **`{TEXT}`**: Replaced by the raw content of the article (the Markdown or text body).

### Examples

Here are some common examples of how to use the `--share` flag:

**For X (formerly Twitter):**

```bash
--share "X:https://twitter.com/intent/tweet?url={URL}&text={TITLE}"
```

**For LinkedIn:**

```bash
--share "LinkedIn:https://www.linkedin.com/sharing/share-offsite/?url={URL}"
```

**For Reddit:**

```bash
--share "Reddit:https://www.reddit.com/submit?url={URL}&title={TITLE}"
```

**For Email:**

```bash
--share "Email:mailto:?subject={TITLE}&body={TEXT}%0A%0A{URL}"
```

**Combining Multiple Providers:**

To include links for X, LinkedIn, and Reddit, simply use the flag for each one:

```bash
./target/release/picoblog \
    --share "X:https://twitter.com/intent/tweet?url={URL}&text={TITLE}" \
    --share "LinkedIn:https://www.linkedin.com/sharing/share-offsite/?url={URL}" \
    --share "Reddit:https://www.reddit.com/submit?url={URL}&title={TITLE}"
```