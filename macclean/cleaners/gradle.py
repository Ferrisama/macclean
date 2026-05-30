from pathlib import Path
import click
from macclean.core.utils import AnalysisResult

def analyze(home: Path | None = None) -> AnalysisResult:
    return AnalysisResult()

def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    pass

@click.command()
@click.pass_context
def cmd(ctx):
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
