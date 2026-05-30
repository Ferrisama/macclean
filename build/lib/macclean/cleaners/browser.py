from macclean.cleaners._dir_cleaners import DirCleanerConfig, make_dir_cleaner

_cfg = DirCleanerConfig(
    name="browser caches", title="Browser Caches",
    dirs=[
        ("Safari",  "Library/Caches/com.apple.Safari"),
        ("Chrome",  "Library/Caches/Google/Chrome"),
        ("Firefox", "Library/Caches/Firefox"),
        ("Edge",    "Library/Caches/Microsoft Edge"),
        ("Brave",   "Library/Caches/BraveSoftware"),
    ],
)
analyze, clean, cmd = make_dir_cleaner(_cfg)
