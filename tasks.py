from pathlib import Path

from invoke import task


@task
def docs(ctx):
    docs = Path(__file__).parent / "docs"
    cli = docs / "cli.md"

    commands = [
        "--help",
        "add --help",
        "run --help",
    ]

    lines = [
        "This is the raw help text for the command line interface.",
    ]
    for command in commands:
        output = ctx.run(f"cargo run -- {command}")
        lines.append("")
        lines.append(f"## `{command}`")
        lines.append("```")
        for line in output.stdout.splitlines():
            lines.append(line.rstrip())
        lines.append("```")

    if not docs.exists():
        docs.mkdir()
    cli.unlink()
    with cli.open("a") as f:
        for line in lines:
            f.write(line + "\n")
