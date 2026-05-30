from macclean.cleaners._dir_cleaners import DirCleanerConfig, make_dir_cleaner

_cfg = DirCleanerConfig(
    name="Node.js caches", title="Node.js Caches",
    dirs=[
        ("npm cache",   ".npm"),
        ("yarn cache",  ".yarn/cache"),
        ("pnpm store",  ".local/share/pnpm/store"),
        ("pnpm cache",  "Library/Caches/pnpm"),
    ],
)
analyze, clean, cmd = make_dir_cleaner(_cfg)
