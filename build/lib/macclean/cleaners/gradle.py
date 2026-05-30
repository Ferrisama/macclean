from macclean.cleaners._dir_cleaners import DirCleanerConfig, make_dir_cleaner

_cfg = DirCleanerConfig(
    name="Gradle caches", title="Gradle Cache",
    dirs=[("Gradle caches", ".gradle/caches")],
)
analyze, clean, cmd = make_dir_cleaner(_cfg)
