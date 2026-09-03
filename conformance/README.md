# Compatibility corpus

Large `.sdocx` fixtures and reference exports live outside this Git repository.
The external dataset is
[`twangodev/sdocx-compatibility`](https://huggingface.co/datasets/twangodev/sdocx-compatibility)
on Hugging Face and is licensed under CC BY 4.0. Check out or download that
dataset into the ignored `hf/` directory at the repository root. A different
checkout can be selected with `SDOCX_CORPUS_DIR`.

The tracked [`corpus.tsv`](corpus.tsv) is the lock file. Each row records the
fixture filenames, SHA-256 digests, and a small set of stable parser/layout
expectations. Store the source `.sdocx` and its Samsung-generated reference PDF
side by side in the dataset repository.

Run the external corpus locally with:

```sh
cargo test -p sdocx --test conformance -- --ignored
```

Or point at an existing dataset checkout:

```sh
SDOCX_CORPUS_DIR=/path/to/dataset cargo test -p sdocx --test conformance -- --ignored
```

Regular unit tests do not download or require private/large fixtures. To add a
fixture, upload its artifacts to the dataset, calculate both SHA-256 digests,
then append one tab-separated row to `corpus.tsv`.

Future automated visual checks can use this same manifest to resolve the exact
fixture and Samsung-generated reference PDF for each compatibility case.
