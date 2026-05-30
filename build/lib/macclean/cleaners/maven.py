from macclean.cleaners._dir_cleaners import DirCleanerConfig, make_dir_cleaner

_cfg = DirCleanerConfig(
    name="Maven local repository", title="Maven Repository",
    dirs=[("Maven local repository", ".m2/repository")],
)
analyze, clean, cmd = make_dir_cleaner(_cfg)
