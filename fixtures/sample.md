# mdedit sample

A Markdown file for the editor's screenshots. The editor shows the *source*, coloured, not the
rendered page; the **viewer** renders it.

## Lists

- First item with `inline code`
- Second item with a [link](https://example.com/docs "Title")
  - Nested item
- [x] A done task
- [ ] An open task

1. One
2. Two
3. Three

## Code

```rust
fn main() {
    println!("Hello, {}!", "world");
}
```

> A quote that runs over more than one line, to show how a long line looks when it does not
> wrap: the editor scrolls horizontally instead.

| Column | Value |
|--------|------:|
| alpha  |     1 |
| beta   |    22 |

Türkçe karakterler: ğüşıöç ĞÜŞİÖÇ, and an emoji: 🙂.

---

Footnote reference[^1].

[^1]: The footnote text.
