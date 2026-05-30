from macclean.cleaners._dir_cleaners import DirCleanerConfig, make_dir_cleaner

_cfg = DirCleanerConfig(
    name="Stremio video cache", title="Stremio Video Cache",
    dirs=[("stremio-server video cache",
           "Library/Application Support/stremio-server/stremio-cache")],
    note="Stremio will re-cache streams on next use.",
)
analyze, clean, cmd = make_dir_cleaner(_cfg)
