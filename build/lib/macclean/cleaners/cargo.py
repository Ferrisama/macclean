from macclean.cleaners._dir_cleaners import DirCleanerConfig, make_dir_cleaner

_cfg = DirCleanerConfig(
    name="Cargo caches", title="Cargo (Rust) Cache",
    dirs=[
        ("registry cache", ".cargo/registry/cache"),
        ("registry src",   ".cargo/registry/src"),
        ("git checkouts",  ".cargo/git/checkouts"),
    ],
)
analyze, clean, cmd = make_dir_cleaner(_cfg)
